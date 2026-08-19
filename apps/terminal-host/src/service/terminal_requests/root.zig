const core_module = @import("terminal-host-core").state;
const host_outgoing = @import("../outgoing.zig");
const host_request_context = @import("../request_context.zig");
const host_response = @import("../response.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");

const attachment_requests = @import("attachments.zig");
const configuration_requests = @import("configuration.zig");
const creation_requests = @import("creation.zig");
const inventory_requests = @import("inventory.zig");
const lifecycle_requests = @import("lifecycle.zig");
const theme_requests = @import("theme.zig");

const Context = host_request_context.Context;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    success_response: host_response.SuccessResponse,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    const success = try host_response.requireControlJsonSuccess(success_response);
    var handler: Handler = .{ .context = context, .success = success };
    switch (operation) {
        .ping => try handler.handlePing(envelope, correlation_id, outgoing),
        .list_terminals => try handler.handleList(envelope, correlation_id, outgoing),
        .create_terminal => try handler.handleCreate(connection_id, envelope, correlation_id, outgoing),
        .start_terminal => try handler.handleStart(connection_id, envelope, correlation_id, outgoing),
        .terminate_terminal => try handler.handleTerminate(connection_id, envelope, correlation_id, outgoing),
        .force_terminate_terminal => try handler.handleForceTerminate(connection_id, envelope, correlation_id, outgoing),
        .dispose_terminal => try handler.handleDispose(connection_id, envelope, correlation_id, outgoing),
        .attach_terminal => try handler.handleAttach(connection_id, envelope, correlation_id, outgoing),
        .detach_terminal => try handler.handleDetach(connection_id, envelope, correlation_id, outgoing),
        .set_terminal_defaults => try handler.handleSetTerminalDefaults(envelope, correlation_id, outgoing),
        .update_terminal_configuration => try handler.handleUpdateTerminalConfiguration(
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        .apply_terminal_theme => try handler.handleApplyTerminalTheme(
            connection_id,
            envelope,
            correlation_id,
            outgoing,
        ),
        else => unreachable,
    }
}

const Handler = struct {
    context: *Context,
    success: host_response.ControlJsonSuccess,

    const handlePing = inventory_requests.handlePing;
    const handleList = inventory_requests.handleList;
    const handleCreate = creation_requests.handleCreate;
    const handleStart = creation_requests.handleStart;
    const handleSetTerminalDefaults = configuration_requests.handleSetTerminalDefaults;
    const handleUpdateTerminalConfiguration = configuration_requests.handleUpdateTerminalConfiguration;
    const handleTerminate = lifecycle_requests.handleTerminate;
    const handleForceTerminate = lifecycle_requests.handleForceTerminate;
    const handleDispose = lifecycle_requests.handleDispose;
    const handleAttach = attachment_requests.handleAttach;
    const handleDetach = attachment_requests.handleDetach;
    const handleApplyTerminalTheme = theme_requests.handleApplyTerminalTheme;
};
