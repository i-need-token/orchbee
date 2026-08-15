const std = @import("std");

const control_encode = @import("terminal-host-control").encode;
const core_module = @import("terminal-host-core").state;
const host_connection = @import("connection_workflow.zig");
const host_interaction = @import("interaction.zig");
const host_outgoing = @import("outgoing.zig");
const host_projection = @import("projection.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate.zig");
const protocol = @import("terminal-protocol");

const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;

const EgressPriority = enum {
    interaction,
    projection,

    fn next(self: EgressPriority) EgressPriority {
        return switch (self) {
            .interaction => .projection,
            .projection => .interaction,
        };
    }
};

pub const OutcomePublicationHandoff = struct {
    count: usize = 0,
    tokens: [64]core_module.OutcomePublicationToken = undefined,

    fn add(
        self: *OutcomePublicationHandoff,
        token: core_module.OutcomePublicationToken,
    ) void {
        std.debug.assert(self.count < self.tokens.len);
        std.debug.assert(!self.contains(token));
        self.tokens[self.count] = token;
        self.count += 1;
    }

    fn contains(
        self: *const OutcomePublicationHandoff,
        token: core_module.OutcomePublicationToken,
    ) bool {
        for (self.tokens[0..self.count]) |scheduled| {
            if (scheduled.kind == token.kind and
                scheduled.final_revision == token.final_revision and
                scheduled.terminal_id.eql(token.terminal_id))
            {
                return true;
            }
        }
        return false;
    }

    pub fn commit(
        self: *OutcomePublicationHandoff,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    ) void {
        runtime_aggregate.commitOutcomePublications(self.tokens[0..self.count]);
        self.count = 0;
    }
};

/// Owns fair asynchronous egress scheduling and the publication handoff that
/// commits outcome visibility only after the complete outgoing batch exists.
pub const Workflow = struct {
    interaction_scheduler: host_interaction.Scheduler = .{},
    next_egress_priority: EgressPriority = .projection,
    projection_scheduler: host_projection.Scheduler = .{},

    pub fn pollEvents(
        self: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection: *const host_connection.ConnectionState,
    ) !Outgoing {
        var outcome_handoff: OutcomePublicationHandoff = .{};
        const outgoing = try self.pollEventsPreparing(
            allocator,
            runtime_aggregate,
            connection,
            &outcome_handoff,
        );
        outcome_handoff.commit(runtime_aggregate);
        return outgoing;
    }

    fn pollEventsPreparing(
        self: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection: *const host_connection.ConnectionState,
        outcome_handoff: *OutcomePublicationHandoff,
    ) !Outgoing {
        var outgoing = Outgoing.init(allocator);
        errdefer outgoing.deinit();
        const active: *const host_connection.ConnectionState.Active = switch (connection.*) {
            .active => |*active| active,
            .awaiting_hello, .closing => return outgoing,
        };
        try self.synchronizeRuntimeLifecycle(
            allocator,
            runtime_aggregate,
            &outgoing,
            outcome_handoff,
        );
        try self.scheduleEgress(allocator, runtime_aggregate, active, &outgoing);
        return outgoing;
    }

    pub fn synchronizeRuntimeLifecycle(
        self: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        outgoing: *Outgoing,
        outcome_handoff: *OutcomePublicationHandoff,
    ) !void {
        runtime_aggregate.synchronizeRuntimes();
        try self.appendPendingOutcomes(
            allocator,
            runtime_aggregate,
            outgoing,
            outcome_handoff,
        );
    }

    fn appendPendingOutcomes(
        _: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        outgoing: *Outgoing,
        outcome_handoff: *OutcomePublicationHandoff,
    ) !void {
        const remaining = outcome_handoff.tokens.len - outcome_handoff.count;
        if (remaining == 0) return;
        const pending_outcomes = runtime_aggregate.pendingOutcomes(remaining);
        for (pending_outcomes.outcomes()) |outcome| {
            const token = outcome.token();
            if (outcome_handoff.contains(token)) continue;
            if (outcome_handoff.count == outcome_handoff.tokens.len) break;
            const payload = switch (outcome) {
                .exited => |notice| try control_encode.terminalExited(
                    allocator,
                    runtime_aggregate.hostEpoch(),
                    notice,
                ),
                .failed => |notice| try control_encode.terminalFailed(
                    allocator,
                    runtime_aggregate.hostEpoch(),
                    notice,
                ),
            };
            defer allocator.free(payload);
            switch (try outgoing.append(
                switch (outcome) {
                    .exited => MessageKind.terminal_exited,
                    .failed => MessageKind.terminal_failed,
                },
                .control_json,
                payload,
                host_response.eventDelivery(runtime_aggregate.hostEpoch()),
            )) {
                .appended => outcome_handoff.add(token),
                .full => break,
            }
        }
    }

    pub fn scheduleEgress(
        self: *Workflow,
        allocator: std.mem.Allocator,
        runtime_aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        connection: *const host_connection.ConnectionState.Active,
        outgoing: *Outgoing,
    ) !void {
        const first = self.next_egress_priority;
        const record_count_before = outgoing.records().len;
        const first_outcome = switch (first) {
            .interaction => try self.interaction_scheduler.schedule(
                allocator,
                runtime_aggregate,
                connection.connection_id,
                outgoing,
            ),
            .projection => try self.projection_scheduler.schedule(
                allocator,
                runtime_aggregate,
                connection.connection_id,
                connection.limits.frame_bytes,
                outgoing,
            ),
        };

        // Once one class emits records, end this dispatch with a coherent
        // scheduler transaction. The other class leads the next dispatch, so
        // a stream of full projection batches cannot starve interactions.
        if (first_outcome == .complete and
            outgoing.records().len == record_count_before)
        {
            _ = switch (first.next()) {
                .interaction => try self.interaction_scheduler.schedule(
                    allocator,
                    runtime_aggregate,
                    connection.connection_id,
                    outgoing,
                ),
                .projection => try self.projection_scheduler.schedule(
                    allocator,
                    runtime_aggregate,
                    connection.connection_id,
                    connection.limits.frame_bytes,
                    outgoing,
                ),
            };
        }
        self.next_egress_priority = first.next();
    }

    pub fn setNextProjectionForTesting(self: *Workflow) void {
        self.next_egress_priority = .projection;
    }
};
