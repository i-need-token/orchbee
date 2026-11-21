//! The answers of `handover` requests, kept until the session that asked takes them. Every request
//! reserves its answer's place when it arrives, so an answer that completes later is never lost;
//! one whose session ended is dropped, as a closed session's answers are. Only the dispatch thread
//! touches them.

const std = @import("std");
const core_module = @import("terminal-host-core").state;
const identifiers = @import("terminal-host-model").identifiers;
const protocol = @import("terminal-protocol");
const host_outgoing = @import("../outgoing.zig");
const host_response = @import("../response.zig");

/// The request an answer goes to.
pub const Pending = struct {
    connection_id: core_module.ConnectionId,
    correlation_id: identifiers.IdentifierBytes,
};

/// The session a delivery writes to.
pub const Session = struct {
    connection_id: core_module.ConnectionId,
    outgoing: *host_outgoing.Outgoing,
};

const Body = union(enum) {
    success: struct { kind: u16, encoding: protocol.PayloadEncoding, payload: []const u8, owned: bool },
    failure: host_response.FailureDetails,
};

const Answer = struct { to: Pending, body: Body };

pub const Answers = struct {
    allocator: std.mem.Allocator,
    list: std.ArrayList(Answer) = .empty,
    /// Requests that arrived and are not answered yet; the list keeps room for each.
    reserved: usize = 0,

    pub fn deinit(self: *Answers) void {
        for (self.list.items) |answer| self.free(answer);
        self.list.deinit(self.allocator);
        self.* = undefined;
    }

    /// Keeps room for one more request's answer.
    pub fn reserve(self: *Answers) !void {
        try self.list.ensureTotalCapacity(self.allocator, self.list.items.len + self.reserved + 1);
        self.reserved += 1;
    }

    /// Gives up a reservation whose request another owner answers.
    pub fn handOver(self: *Answers) void {
        self.reserved -= 1;
    }

    /// Answers `to` with a payload this list then owns; nothing when `to` is null.
    pub fn success(self: *Answers, to: ?Pending, kind: u16, encoding: protocol.PayloadEncoding, payload: []u8) void {
        self.add(to, .{ .success = .{ .kind = kind, .encoding = encoding, .payload = payload, .owned = true } });
    }

    /// Answers `to` with a control payload the Host fixes.
    pub fn successStatic(self: *Answers, to: ?Pending, kind: u16, payload: []const u8) void {
        self.add(to, .{ .success = .{ .kind = kind, .encoding = .control_json, .payload = payload, .owned = false } });
    }

    pub fn failure(self: *Answers, to: ?Pending, details: host_response.FailureDetails) void {
        self.add(to, .{ .failure = details });
    }

    /// Writes every answer to `session` and drops those of any other session. An answer that does
    /// not fit stays for the next delivery.
    pub fn deliver(self: *Answers, session: ?Session, epoch: identifiers.HostEpoch) void {
        while (self.list.items.len != 0) {
            const answer = self.list.items[0];
            if (session) |current| if (current.connection_id == answer.to.connection_id) {
                self.write(answer, current.outgoing, epoch) catch return;
            };
            self.free(answer);
            _ = self.list.orderedRemove(0);
        }
    }

    fn add(self: *Answers, to: ?Pending, body: Body) void {
        const target = to orelse return self.free(.{ .to = undefined, .body = body });
        std.debug.assert(self.reserved != 0);
        self.reserved -= 1;
        self.list.appendAssumeCapacity(.{ .to = target, .body = body });
    }

    fn write(self: *Answers, answer: Answer, outgoing: *host_outgoing.Outgoing, epoch: identifiers.HostEpoch) !void {
        const delivery = host_response.responseDelivery(answer.to.correlation_id, epoch);
        switch (answer.body) {
            .success => |value| _ = try host_response.appendRequired(outgoing, value.kind, value.encoding, value.payload, delivery),
            .failure => |details| try host_response.appendFailure(self.allocator, outgoing, details, delivery),
        }
    }

    fn free(self: *Answers, answer: Answer) void {
        switch (answer.body) {
            .success => |value| if (value.owned) self.allocator.free(value.payload),
            .failure => {},
        }
    }
};
