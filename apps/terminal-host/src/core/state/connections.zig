const target_builtin = @import("builtin");
const broker_session = @import("../broker_session.zig");
const contracts = @import("contracts.zig");

const AttachmentId = contracts.AttachmentId;
const ClientId = contracts.ClientId;
const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const TransitionOutcome = contracts.TransitionOutcome;
const BrokerSession = broker_session.Session;

pub fn connect(
    self: anytype,
    connection_id: ConnectionId,
    client_id: ClientId,
) CoreError!TransitionOutcome {
    if (self.broker) |broker| {
        return if (broker.matches(connection_id, client_id))
            .idempotent
        else
            error.ConnectionConflict;
    }
    self.broker = BrokerSession.init(self.allocator, connection_id, client_id);
    return .applied;
}

pub fn disconnect(self: anytype, connection_id: ConnectionId) TransitionOutcome {
    const broker = if (self.broker) |*active| active else return .idempotent;
    if (!broker.owns(connection_id)) return .idempotent;
    if (target_builtin.is_test) {
        self.testing_state.last_active_read_demand_scans =
            broker_session.testing.activeReadDemandScans(broker);
    }
    broker.deinit();
    self.broker = null;
    return .applied;
}

pub fn attachmentsForConnection(self: anytype, connection_id: ConnectionId) []const AttachmentId {
    const broker = if (self.broker) |*active| active else return &.{};
    return if (broker.owns(connection_id))
        broker.keys()
    else
        &.{};
}

pub fn connectionCount(self: anytype) usize {
    return @intFromBool(self.broker != null);
}

pub fn connectionClientId(self: anytype, connection_id: ConnectionId) ?ClientId {
    const broker = self.broker orelse return null;
    return if (broker.owns(connection_id)) broker.clientId() else null;
}
