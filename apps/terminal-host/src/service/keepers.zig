//! The `keeper` capability of the Worker session: `AttachKeeper` and `DetachKeeper`, the hold of
//! every running terminal in each attached Keeper, the answers and events the Keeper links report,
//! and each held terminal's `holdingId` in the inventory. Only the dispatch thread calls it. An
//! attach or detach answers once its link reports, so a reply waits for a Keeper connect or for
//! the releases, never for the dispatch thread.

const std = @import("std");
const builtin = @import("builtin");
const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");

const host_outgoing = @import("outgoing.zig");
const host_request_context = @import("request_context.zig");
const host_response = @import("response.zig");
const host_runtime_aggregate = @import("runtime_aggregate/root.zig");
const identifiers = @import("terminal-host-model").identifiers;

const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const Keeper = keeper_link.Keeper;

/// How often the dispatch thread offers running terminals to the attached Keepers.
const offer_interval_ns = 50 * std.time.ns_per_ms;

const unavailable_failure: host_response.FailureDetails = .{
    .code = "keeper-unavailable",
    .message = "The Host could not check or connect to the Keeper",
    .retryable = false,
};

const Pending = struct {
    connection_id: core_module.ConnectionId,
    correlation_id: WireIdentifier,
    keeper: Keeper,
    request: Request,
};

const Request = enum { attach, detach };

/// The Worker session a delivery may answer and send events to.
pub const Session = struct {
    connection_id: core_module.ConnectionId,
    outgoing: *Outgoing,
};

pub const Keepers = struct {
    allocator: std.mem.Allocator,
    attachments: keeper_link.Attachments,
    /// Whether the current Worker session negotiated `keeper`.
    negotiated: bool = false,
    pending: std.ArrayList(Pending) = .empty,
    /// Notices taken from the links and not yet applied.
    notices: std.ArrayList(keeper_link.Notice) = .empty,
    timer: ?platform.monotonic.Timer,
    last_offer_ns: ?u64 = null,
    stopped: bool = false,

    pub fn init(allocator: std.mem.Allocator) Keepers {
        return .{
            .allocator = allocator,
            .attachments = .init(allocator),
            .timer = platform.monotonic.Timer.start() catch null,
        };
    }

    /// Host teardown, once its dispatch loop has ended: starts every terminal's release first at
    /// once, so the links release in parallel, then waits for them, each bounded by its Keeper's
    /// reply deadline. The shutdowns that follow then never wait for a release.
    pub fn releaseAll(self: *Keepers, aggregate: *host_runtime_aggregate.HostRuntimeAggregate) void {
        if (comptime builtin.os.tag != .windows) return;
        if (self.stopped) return;
        const sessions = aggregate.coreRead().terminalValues();
        for (sessions) |session| if (gateOf(aggregate, session.terminal_id)) |gate| {
            _ = gate.tryPass();
        };
        for (sessions) |session| if (gateOf(aggregate, session.terminal_id)) |gate| gate.pass();
    }

    /// Stops and joins every link once every terminal has ended. Repeating it changes nothing.
    pub fn stop(self: *Keepers) void {
        if (self.stopped) return;
        self.stopped = true;
        self.attachments.deinit();
        self.pending.deinit(self.allocator);
        self.notices.deinit(self.allocator);
    }

    /// A Worker session was admitted; `negotiated` tells whether both sides listed `keeper`.
    pub fn admit(self: *Keepers, negotiated: bool) void {
        self.negotiated = negotiated;
    }

    /// The session `connection_id` ended: its unanswered requests go unanswered.
    pub fn disconnect(self: *Keepers, connection_id: ?core_module.ConnectionId) void {
        self.negotiated = false;
        if (self.stopped) return;
        var index: usize = 0;
        while (index < self.pending.items.len) {
            if (connection_id == null or self.pending.items[index].connection_id == connection_id.?) {
                _ = self.pending.orderedRemove(index);
            } else index += 1;
        }
    }

    /// The request context's view: present only for a session that negotiated `keeper`.
    pub fn forSession(self: *Keepers) ?*Keepers {
        return if (self.negotiated and !self.stopped) self else null;
    }

    pub fn handle(
        self: *Keepers,
        context: *host_request_context.Context,
        operation: protocol.RequestOperation,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try context.requireControl(envelope, correlation_id, outgoing)) return;
        const delivery = host_response.responseDelivery(correlation_id, context.hostEpoch());
        try self.pending.ensureUnusedCapacity(self.allocator, 1);
        switch (operation) {
            .attach_keeper => {
                var decoded = control.decode.decodeAttachKeeper(context.allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "AttachKeeper payload is invalid");
                defer decoded.deinit();
                const keeper: Keeper = .{ .pid = decoded.value.keeper.pid, .created = decoded.value.keeper.created };
                switch (self.attachments.attach(keeper, decoded.value.endpoint_id)) {
                    .attached => try answerAttach(context.allocator, outgoing, keeper, delivery),
                    .pending => self.pending.appendAssumeCapacity(.{
                        .connection_id = connection_id,
                        .correlation_id = correlation_id,
                        .keeper = keeper,
                        .request = .attach,
                    }),
                    .limit => try host_response.appendFailure(context.allocator, outgoing, .{
                        .code = "keeper-limit",
                        .message = "The Host already holds two Keeper attachments",
                        .retryable = false,
                    }, delivery),
                    .unavailable => try host_response.appendFailure(context.allocator, outgoing, unavailable_failure, delivery),
                }
                self.last_offer_ns = null;
            },
            .detach_keeper => {
                var decoded = control.decode.decodeDetachKeeper(context.allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "DetachKeeper payload is invalid");
                defer decoded.deinit();
                const keeper: Keeper = .{ .pid = decoded.value.pid, .created = decoded.value.created };
                switch (self.attachments.detach(keeper)) {
                    .done => try answerDetach(outgoing, delivery),
                    .pending => self.pending.appendAssumeCapacity(.{
                        .connection_id = connection_id,
                        .correlation_id = correlation_id,
                        .keeper = keeper,
                        .request = .detach,
                    }),
                }
            },
            else => unreachable,
        }
    }

    /// Offers every running terminal to the attached Keepers, at most once per interval.
    pub fn offer(self: *Keepers, aggregate: *host_runtime_aggregate.HostRuntimeAggregate) void {
        if (comptime builtin.os.tag != .windows) return;
        if (self.stopped or !self.attachments.any()) return;
        if (self.timer) |*timer| {
            const now = timer.read();
            if (self.last_offer_ns) |last| if (now -| last < offer_interval_ns) return;
            self.last_offer_ns = now;
        }
        const epoch = aggregate.hostEpoch();
        for (aggregate.coreRead().terminalValues()) |session| {
            if (session.phase() != .running or session.terminateRequested()) continue;
            const runtime = aggregate.runtimeFor(session.terminal_id) orelse continue;
            self.attachments.offer(runtime.keeperSubject(), .{
                .creation_id = session.creation_id.bytes,
                .host_epoch = epoch.bytes,
                .terminal_id = session.terminal_id.bytes,
            });
        }
    }

    /// Applies what the links reported: answers to `session`'s requests and, when it negotiated
    /// `keeper`, the events. Without a session the events are dropped. A notice that does not fit
    /// in `outgoing` stays for the next delivery.
    pub fn deliver(
        self: *Keepers,
        aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
        session: ?Session,
    ) void {
        if (self.stopped) return;
        self.offer(aggregate);
        self.attachments.poll(&self.notices);
        const epoch = aggregate.hostEpoch();
        var applied: usize = 0;
        defer self.notices.replaceRangeAssumeCapacity(0, applied, &.{});
        for (self.notices.items) |notice| {
            self.apply(notice, epoch, session) catch break;
            applied += 1;
        }
    }

    fn apply(self: *Keepers, notice: keeper_link.Notice, epoch: identifiers.HostEpoch, session: ?Session) !void {
        switch (notice) {
            .attached => |keeper| try self.answer(.attach, keeper, .success, epoch, session),
            .unavailable => |keeper| {
                try self.answer(.attach, keeper, .failure, epoch, session);
                try self.answer(.detach, keeper, .success, epoch, session);
            },
            .detached => |keeper| try self.answer(.detach, keeper, .success, epoch, session),
            .lost => |keeper| {
                try self.answer(.attach, keeper, .failure, epoch, session);
                try self.answer(.detach, keeper, .success, epoch, session);
                try self.event(MessageKind.keeper_lost, keeper, null, epoch, session);
            },
            .unresponsive => |keeper| try self.event(MessageKind.keeper_unresponsive, keeper, null, epoch, session),
            .unheld => |unheld| try self.event(MessageKind.terminal_unheld, unheld.keeper, unheld, epoch, session),
        }
    }

    /// Answers every pending `request` naming `keeper`; one of another session goes unanswered.
    fn answer(
        self: *Keepers,
        request: Request,
        keeper: Keeper,
        outcome: enum { success, failure },
        epoch: identifiers.HostEpoch,
        session: ?Session,
    ) !void {
        var index: usize = 0;
        while (index < self.pending.items.len) {
            const pending = self.pending.items[index];
            if (pending.request != request or !pending.keeper.eql(keeper)) {
                index += 1;
                continue;
            }
            if (session) |current| if (current.connection_id == pending.connection_id) {
                const delivery = host_response.responseDelivery(pending.correlation_id, epoch);
                switch (request) {
                    .attach => switch (outcome) {
                        .success => try answerAttach(self.allocator, current.outgoing, keeper, delivery),
                        .failure => try host_response.appendFailure(self.allocator, current.outgoing, unavailable_failure, delivery),
                    },
                    .detach => try answerDetach(current.outgoing, delivery),
                }
            };
            _ = self.pending.orderedRemove(index);
        }
    }

    fn event(
        self: *Keepers,
        kind: u16,
        keeper: Keeper,
        unheld: ?keeper_link.Unheld,
        epoch: identifiers.HostEpoch,
        session: ?Session,
    ) !void {
        const current = session orelse return;
        if (!self.negotiated) return;
        const named: control.encode.KeeperProcess = .{ .pid = keeper.pid, .created = keeper.created };
        const payload = if (unheld) |value|
            try control.encode.terminalUnheld(self.allocator, named, identifiers.TerminalId.fromBytes(value.terminal_id), value.reason)
        else
            try control.encode.keeperNamed(self.allocator, named);
        defer self.allocator.free(payload);
        _ = try host_response.appendRequired(current.outgoing, kind, .control_json, payload, host_response.eventDelivery(epoch));
    }
};

fn answerAttach(allocator: std.mem.Allocator, outgoing: *Outgoing, keeper: Keeper, delivery: host_outgoing.Delivery) !void {
    const payload = try control.encode.keeperNamed(allocator, .{ .pid = keeper.pid, .created = keeper.created });
    defer allocator.free(payload);
    _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.attach_keeper }, payload, delivery);
}

fn answerDetach(outgoing: *Outgoing, delivery: host_outgoing.Delivery) !void {
    _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.detach_keeper }, control.encode.empty_control, delivery);
}

/// Each held terminal's holding id, for the inventory of a session that negotiated `keeper`.
pub const Holdings = struct {
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,

    pub fn holdingId(self: Holdings, terminal_id: core_module.TerminalId) ?[16]u8 {
        if (comptime builtin.os.tag != .windows) return null;
        const gate = gateOf(self.aggregate, terminal_id) orelse return null;
        return gate.heldId();
    }
};

fn gateOf(aggregate: *host_runtime_aggregate.HostRuntimeAggregate, terminal_id: core_module.TerminalId) ?*platform.keeper.Gate {
    const runtime = aggregate.runtimeFor(terminal_id) orelse return null;
    return platform.keeper.gate(runtime.keeperSubject());
}
