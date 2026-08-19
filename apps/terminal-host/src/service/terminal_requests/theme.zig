const control = @import("terminal-host-control");
const control_decode = control.decode;
const control_encode = control.encode;
const core_module = @import("terminal-host-core").state;
const host_failure = @import("../failure.zig");
const host_outgoing = @import("../outgoing.zig");
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");

const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;

pub fn handleApplyTerminalTheme(
    self: anytype,
    connection_id: core_module.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: WireIdentifier,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control_decode.decodeApplyTerminalTheme(
        self.context.allocator,
        envelope.payload,
    ) catch |err|
        return self.context.rejectControlDecode(
            outgoing,
            correlation_id,
            err,
            "ApplyTerminalTheme payload is invalid",
        );
    defer decoded.deinit();

    // Reserve the complete response before the runtime effect. Once the
    // aggregate commits, encoding and publication cannot allocate or fail.
    var response = try @import("../response.zig").prepareControl(
        outgoing,
        correlation_id,
        self.context.hostEpoch(),
    );
    defer response.deinit();
    const result = self.context.runtime_aggregate.applyTerminalTheme(
        connection_id,
        decoded.value.terminal_id,
        decoded.value.operation_id,
        decoded.value.expected_theme_revision,
        decoded.value.terminal,
    ) catch |err| {
        response.commitFailure(switch (err) {
            error.RuntimeStateDiverged, error.TerminalRuntimeMissing => host_failure.runtime_state_diverged,
            error.TerminalRevisionOverflow => .{
                .code = "resource-exhausted",
                .message = "terminal revision space is exhausted",
                .retryable = false,
            },
            error.TerminalRuntimeUnavailable => host_failure.fromCore(error.InvalidPhase),
            else => host_failure.fromCore(@errorCast(err)),
        });
        return;
    };
    var output = response.writer();
    control_encode.writeTerminalThemeResult(&output, result) catch unreachable;
    _ = response.commitControlSuccess(self.success, &output);
}
