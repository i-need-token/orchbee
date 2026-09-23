const control = @import("terminal-host-control");
const core = @import("terminal-host-core").state;
const protocol = @import("terminal-protocol");
const identifiers = @import("terminal-host-model").identifiers;
const response = @import("../response.zig");
const Outgoing = @import("../outgoing.zig").Outgoing;

pub fn handleReadProcessInfo(
    self: anytype,
    connection_id: core.ConnectionId,
    envelope: protocol.EnvelopeSummary,
    correlation_id: identifiers.IdentifierBytes,
    outgoing: *Outgoing,
) !void {
    if (!try self.context.requireControl(envelope, correlation_id, outgoing)) return;
    var decoded = control.decode.decodeReadProcessInfo(self.context.allocator, envelope.payload) catch |err|
        return self.context.rejectControlDecode(outgoing, correlation_id, err, "ReadProcessInfo payload is invalid");
    defer decoded.deinit();
    var observed = self.context.runtime_aggregate.sampleTerminalProcesses(
        self.context.allocator,
        connection_id,
        decoded.value,
    ) catch |err| {
        const code: []const u8 = switch (err) {
            error.InteractionAccessUnavailable, error.TerminalRuntimeMissing => "missing-resource",
            error.ProcessSampleUnsupported => "process-sample-unsupported",
            error.ProcessSampleLimit => "limit-exceeded",
            error.ProcessSampleChanged => "process-sample-changed",
            error.ProcessSampleUnavailable => "process-sample-unavailable",
            else => return err,
        };
        try self.context.addFailure(outgoing, correlation_id, code, "terminal process sample is unavailable", false);
        return;
    };
    defer observed.sample.deinit();
    const payload = control.encode.processInfo(
        self.context.allocator,
        decoded.value,
        observed.terminal_id,
        observed.sample,
    ) catch |err| switch (err) {
        error.ProcessInfoTooLarge => {
            try self.context.addFailure(outgoing, correlation_id, "limit-exceeded", "terminal process sample exceeds the control payload bound", false);
            return;
        },
        else => return err,
    };
    defer self.context.allocator.free(payload);
    _ = try response.appendControlSuccess(outgoing, self.success, payload, response.responseDelivery(correlation_id, self.context.hostEpoch()));
}
