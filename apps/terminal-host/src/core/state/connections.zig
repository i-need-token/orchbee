const std = @import("std");
const target_builtin = @import("builtin");
const broker_session = @import("../broker_session.zig");
const contracts = @import("contracts.zig");

const AttachmentId = contracts.AttachmentId;
const ClientId = contracts.ClientId;
const ConnectionId = contracts.ConnectionId;
const CoreError = contracts.CoreError;
const TransitionOutcome = contracts.TransitionOutcome;
const BrokerSession = broker_session.Session;

const TestingWriteAccess = if (target_builtin.is_test) struct {
    last_active_read_demand_scans: *usize,
} else struct {};

/// Ephemeral read authority supplied by the concrete HostCore owner. It is
/// never retained or exported through the Core boundary.
pub const ReadAccess = struct {
    broker: *const ?BrokerSession,
};

/// Ephemeral mutation authority supplied by the concrete HostCore owner. The
/// pointers keep broker ownership and test observation storage in HostCore.
pub const WriteAccess = struct {
    allocator: std.mem.Allocator,
    broker: *?BrokerSession,
    testing: TestingWriteAccess,
};

pub fn connect(
    access: WriteAccess,
    connection_id: ConnectionId,
    client_id: ClientId,
) CoreError!TransitionOutcome {
    if (access.broker.*) |broker| {
        return if (broker.matches(connection_id, client_id))
            .idempotent
        else
            error.ConnectionConflict;
    }
    access.broker.* = BrokerSession.init(access.allocator, connection_id, client_id);
    return .applied;
}

pub fn disconnect(access: WriteAccess, connection_id: ConnectionId) TransitionOutcome {
    const broker = if (access.broker.*) |*active| active else return .idempotent;
    if (!broker.owns(connection_id)) return .idempotent;
    if (target_builtin.is_test) {
        access.testing.last_active_read_demand_scans.* =
            broker_session.testing.activeReadDemandScans(broker);
    }
    broker.deinit();
    access.broker.* = null;
    return .applied;
}

pub fn attachmentsForConnection(
    access: ReadAccess,
    connection_id: ConnectionId,
) []const AttachmentId {
    const broker = if (access.broker.*) |*active| active else return &.{};
    return if (broker.owns(connection_id))
        broker.keys()
    else
        &.{};
}

pub fn connectionCount(access: ReadAccess) usize {
    return @intFromBool(access.broker.* != null);
}

pub fn connectionClientId(access: ReadAccess, connection_id: ConnectionId) ?ClientId {
    const broker = if (access.broker.*) |*active| active else return null;
    return if (broker.owns(connection_id)) broker.clientId() else null;
}
