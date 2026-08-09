const std = @import("std");

const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("failure.zig");
const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const model = @import("terminal-host-model");
const identifiers = model.identifiers;
const protocol = @import("terminal-protocol");
const terminal_configuration = model.configuration;

const Context = host_request_context.Context;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

pub fn dispatch(
    context: *Context,
    operation: protocol.RequestOperation,
    connection_id: core_module.ConnectionId,
    terminal_defaults: *terminal_configuration.Configuration,
    terminal_defaults_revision: *u64,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    var handler: Handler = .{
        .context = context,
        .terminal_defaults = terminal_defaults,
        .terminal_defaults_revision = terminal_defaults_revision,
    };
    switch (operation) {
        .ping => try handler.handlePing(envelope, correlation_id, outgoing),
        .list_terminals => try handler.handleList(envelope, correlation_id, outgoing),
        .create_terminal => try handler.handleCreate(connection_id, envelope, correlation_id, outgoing),
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
        else => unreachable,
    }
}

const Handler = struct {
    context: *Context,
    terminal_defaults: *terminal_configuration.Configuration,
    terminal_defaults_revision: *u64,

    fn handlePing(
        self: *Handler,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireEmptyControl(envelope, correlation_id, outgoing)) return;
        _ = try host_response.appendRequired(
            outgoing,
            MessageKind.ping,
            .control_json,
            control_encode.empty_control,
            host_response.responseDelivery(correlation_id, self.context.core().epoch),
        );
    }

    fn handleList(
        self: *Handler,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeListTerminals(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "ListTerminals payload is invalid",
            );
        defer decoded.deinit();
        const request = decoded.value;
        const offset = if (request.cursor) |cursor| cursor.offset else 0;
        if (request.cursor) |cursor| {
            if (cursor.generation != self.context.core().inventoryGeneration()) {
                try self.context.addFailure(
                    outgoing,
                    correlation_id,
                    "stale-inventory-cursor",
                    "terminal inventory membership changed",
                    true,
                );
                return;
            }
        }
        if (offset > self.context.core().terminalCount()) {
            try self.context.addFailure(
                outgoing,
                correlation_id,
                "invalid-field",
                "inventory offset exceeds terminal count",
                false,
            );
            return;
        }
        self.context.runtime_aggregate.synchronizeTerminalRange(offset, request.maximum_terminals);
        const payload = control_encode.terminalList(
            self.context.allocator,
            self.context.core().epoch,
            self.context.core().inventoryGeneration(),
            self.context.core().terminalValues(),
            offset,
            request.maximum_terminals,
        ) catch |err| switch (err) {
            error.InventoryTerminalTooLarge => {
                try self.context.addFailure(
                    outgoing,
                    correlation_id,
                    "limit-exceeded",
                    "one terminal inventory summary exceeds the control payload limit",
                    false,
                );
                return;
            },
            else => return err,
        };
        defer self.context.allocator.free(payload);
        _ = try host_response.appendRequired(
            outgoing,
            MessageKind.list_terminals,
            .control_json,
            payload,
            host_response.responseDelivery(correlation_id, self.context.core().epoch),
        );
    }

    fn handleCreate(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeCreateTerminal(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "CreateTerminal payload is invalid",
            );
        defer decoded.deinit();

        var preparation = self.context.runtime_aggregate.prepareCreateTerminal(
            connection_id,
            decoded.value.creation_id,
            decoded.value.launch,
            self.terminal_defaults.*,
        ) catch |err| {
            switch (err) {
                error.LaunchPreparationOutOfMemory => try self.context.addFailure(
                    outgoing,
                    correlation_id,
                    "resource-exhausted",
                    "Host could not retain the terminal environment",
                    true,
                ),
                else => try self.context.addCoreFailure(
                    outgoing,
                    correlation_id,
                    @errorCast(err),
                ),
            }
            return;
        };
        defer preparation.deinit();
        const identity = preparation.identity();
        const payload = try control_encode.terminalIdentity(
            self.context.allocator,
            identity.creation_id,
            self.context.core().epoch,
            identity.terminal_id,
        );
        defer self.context.allocator.free(payload);
        const response_token = try host_response.appendRequired(
            outgoing,
            MessageKind.create_terminal,
            .control_json,
            payload,
            host_response.responseDelivery(correlation_id, self.context.core().epoch),
        );
        if (!preparation.requiresEffect()) return;
        const started_token = host_response.appendRequired(
            outgoing,
            MessageKind.terminal_started,
            .control_json,
            payload,
            host_response.eventDelivery(self.context.core().epoch),
        ) catch |err| {
            outgoing.rollback(response_token) catch unreachable;
            return err;
        };
        const creation = self.context.runtime_aggregate.commitPreparedTerminal(&preparation) catch |err| {
            outgoing.rollback(started_token) catch unreachable;
            outgoing.rollback(response_token) catch unreachable;
            switch (err) {
                error.RuntimeAllocationOutOfMemory => try self.context.addFailure(
                    outgoing,
                    correlation_id,
                    "resource-exhausted",
                    "Host could not allocate terminal runtime state",
                    true,
                ),
                error.RuntimeSpawnFailed => try self.context.addFailure(
                    outgoing,
                    correlation_id,
                    "spawn-failed",
                    "PTY or child creation failed",
                    true,
                ),
            }
            return;
        };
        std.debug.assert(creation.isCreated());
    }

    fn handleSetTerminalDefaults(
        self: *Handler,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeSetTerminalDefaults(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "terminal configuration is invalid",
            );
        defer decoded.deinit();
        const configuration = decoded.value;
        if (configuration.scrollback_bytes != self.terminal_defaults.scrollback_bytes) {
            if (self.terminal_defaults_revision.* == std.math.maxInt(u64)) {
                try self.context.addFailure(
                    outgoing,
                    correlation_id,
                    "resource-exhausted",
                    "terminal default revision space is exhausted",
                    false,
                );
                return;
            }
        }
        const next_revision = if (configuration.scrollback_bytes == self.terminal_defaults.scrollback_bytes)
            self.terminal_defaults_revision.*
        else
            self.terminal_defaults_revision.* + 1;
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        var output = response.writer();
        control_encode.writeTerminalDefaultsAccepted(
            &output,
            configuration.scrollback_bytes,
            next_revision,
        ) catch unreachable;
        self.terminal_defaults.* = configuration;
        self.terminal_defaults_revision.* = next_revision;
        _ = response.commitControl(MessageKind.terminal_defaults_accepted, &output);
    }

    fn handleUpdateTerminalConfiguration(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeUpdateTerminalConfiguration(
            self.context.allocator,
            envelope.payload,
        ) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "UpdateTerminalConfiguration payload is invalid",
            );
        defer decoded.deinit();
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        const update = self.context.runtime_aggregate.updateTerminalConfiguration(
            connection_id,
            decoded.value.terminal_id,
            decoded.value.configuration,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.ConfigurationRevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal configuration revision space is exhausted", .retryable = false },
                error.InvalidConfiguration => .{ .code = "invalid-field", .message = "terminal configuration is invalid", .retryable = false },
                error.RuntimeStateDiverged => host_failure.runtime_state_diverged,
                error.TerminalRevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal revision space is exhausted", .retryable = false },
                error.TerminalRuntimeMissing => unreachable,
                error.TerminalRuntimeUnavailable => host_failure.fromCore(error.InvalidPhase),
                else => host_failure.fromCore(@errorCast(err)),
            });
            return;
        };
        var output = response.writer();
        control_encode.writeTerminalConfigurationAccepted(&output, update) catch unreachable;
        _ = response.commitControl(MessageKind.terminal_configuration_accepted, &output);
    }

    fn handleTerminate(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeTerminalTarget(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "TerminateTerminal payload is invalid",
            );
        defer decoded.deinit();
        const terminal_id = decoded.value;
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        _ = self.context.runtime_aggregate.requestTermination(
            connection_id,
            terminal_id,
            .graceful,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.TerminalRuntimeMissing => unreachable,
                error.TerminationDispatchFailed => .{ .code = "termination-failed", .message = "platform termination request failed", .retryable = true },
                else => host_failure.fromCore(@errorCast(err)),
            });
            return;
        };
        var output = response.writer();
        control_encode.writeTerminalAcknowledged(&output, terminal_id) catch unreachable;
        _ = response.commitControl(MessageKind.terminate_terminal, &output);
    }

    fn handleForceTerminate(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeTerminalTarget(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "ForceTerminateTerminal payload is invalid",
            );
        defer decoded.deinit();
        const terminal_id = decoded.value;
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        _ = self.context.runtime_aggregate.requestTermination(
            connection_id,
            terminal_id,
            .force,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.TerminalRuntimeMissing => unreachable,
                error.TerminationDispatchFailed => .{ .code = "termination-failed", .message = "platform force-termination request failed", .retryable = true },
                else => host_failure.fromCore(@errorCast(err)),
            });
            return;
        };
        var output = response.writer();
        control_encode.writeTerminalAcknowledged(&output, terminal_id) catch unreachable;
        _ = response.commitControl(MessageKind.force_terminate_terminal, &output);
    }

    fn handleDispose(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeTerminalTarget(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "DisposeTerminal payload is invalid",
            );
        defer decoded.deinit();
        const terminal_id = decoded.value;
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        const disposal = self.context.runtime_aggregate.disposeTerminal(
            connection_id,
            terminal_id,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.TerminalRuntimeMissing => unreachable,
                else => host_failure.fromCore(@errorCast(err)),
            });
            return;
        };
        switch (disposal) {
            .transition => {},
            .cleanup_blocked => |failure| {
                response.commitFailure(host_failure.fromCleanup(failure));
                return;
            },
        }
        var output = response.writer();
        control_encode.writeTerminalAcknowledged(&output, terminal_id) catch unreachable;
        _ = response.commitControl(MessageKind.dispose_terminal, &output);
    }

    fn handleAttach(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeAttachTerminal(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "AttachTerminal payload is invalid",
            );
        defer decoded.deinit();
        const request = decoded.value;
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        const attached = self.context.runtime_aggregate.attach(
            connection_id,
            request.attachment_id,
            request.terminal_id,
            request.capabilities,
            request.request_resize_lease,
        ) catch |err| {
            response.commitFailure(switch (err) {
                error.TerminalRuntimeMissing => unreachable,
                error.TerminalRuntimeUnavailable => host_failure.fromCore(error.InvalidPhase),
                error.HistoryAttachmentFailed => .{ .code = "resource-exhausted", .message = "history attachment state could not be allocated", .retryable = true },
                else => host_failure.fromCore(@errorCast(err)),
            });
            return;
        };
        const attachment = attached.binding;
        var output = response.writer();
        control_encode.writeAttachmentCapabilities(&output, attachment) catch unreachable;
        _ = response.commitControl(MessageKind.attachment_capabilities, &output);
    }

    fn handleDetach(
        self: *Handler,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
        var decoded = control_decode.decodeDetachTerminal(self.context.allocator, envelope.payload) catch |err|
            return self.context.rejectControlDecode(
                outgoing,
                correlation_id,
                err,
                "DetachTerminal payload is invalid",
            );
        defer decoded.deinit();
        const attachment_id = decoded.value;
        var response = try host_response.prepareControl(
            outgoing,
            correlation_id,
            self.context.core().epoch,
        );
        defer response.deinit();
        _ = self.context.runtime_aggregate.detach(connection_id, attachment_id) catch |err| {
            response.commitFailure(host_failure.fromCore(err));
            return;
        };
        var output = response.writer();
        control_encode.writeAttachmentAcknowledged(&output, attachment_id) catch unreachable;
        _ = response.commitControl(MessageKind.detach_terminal, &output);
    }
};
