const std = @import("std");

const core_module = @import("terminal-host-core").state;
const host_interaction_requests = @import("interaction_requests.zig");
const host_io_requests = @import("io_requests.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const host_terminal_requests = @import("terminal_requests.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const terminal_configuration = model.configuration;

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
    terminal_defaults: *terminal_configuration.Configuration,
    terminal_defaults_revision: *u64,
    trace_interactions: bool,

    pub fn init(
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        terminal_defaults: *terminal_configuration.Configuration,
        terminal_defaults_revision: *u64,
        trace_interactions: bool,
    ) Dispatcher {
        return .{ .context = .{
            .allocator = allocator,
            .runtime_aggregate = runtime_aggregate,
        }, .terminal_defaults = terminal_defaults, .terminal_defaults_revision = terminal_defaults_revision, .trace_interactions = trace_interactions };
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
        switch (request_contract.operation) {
            .hello => unreachable,
            .ping,
            .list_terminals,
            .create_terminal,
            .terminate_terminal,
            .force_terminate_terminal,
            .dispose_terminal,
            .attach_terminal,
            .detach_terminal,
            .set_terminal_defaults,
            .update_terminal_configuration,
            => try host_terminal_requests.dispatch(
                &self.context,
                request_contract.operation,
                connection_id,
                self.terminal_defaults,
                self.terminal_defaults_revision,
                envelope,
                correlation_id,
                outgoing,
            ),
            .write_input,
            .semantic_input,
            .resize_terminal,
            .frame_applied,
            .need_snapshot,
            .set_projection_demand,
            => try host_io_requests.dispatch(
                &self.context,
                request_contract,
                connection_id,
                limits.input_bytes,
                envelope,
                correlation_id,
                outgoing,
            ),
            .read_rows,
            .rows_applied,
            .set_history_demand,
            .history_observed,
            .selection_gesture,
            .selection_command,
            .selection_export,
            .selection_export_chunk_applied,
            .selection_export_cancel,
            => try host_interaction_requests.dispatch(
                &self.context,
                request_contract.operation,
                connection_id,
                limits.row_window_bytes,
                self.trace_interactions,
                envelope,
                correlation_id,
                outgoing,
            ),
        }
    }
};
