//! `RecoverProgram`: a background program a Keeper kept after its Host ended. Through the Host's one
//! Keeper link the dispatch thread inspects the holding, checks the handles it pulled, claims the
//! holding, and then follows the program as one it started, its captures restarted at zero. The
//! link thread makes every Keeper request and the dispatch thread polls each, so it never waits on
//! a Keeper. A check that fails declines the holding, which leaves the program running there.

const std = @import("std");
const builtin = @import("builtin");
const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const host_outgoing = @import("../outgoing.zig");
const host_response = @import("../response.zig");
const refusal = @import("../moves/refusal.zig");
const identifiers = @import("terminal-host-model").identifiers;
const entry_module = @import("entry.zig");

const Entry = entry_module.Entry;
const is_windows = builtin.os.tag == .windows;
const posix_keeper = builtin.os.tag == .macos or builtin.os.tag == .linux;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const Keeper = keeper_link.Keeper;
const decode = control.decode.program;
const encode = control.encode.program;
const moves = keeper_link.moves;

pub const Session = struct {
    connection_id: core_module.ConnectionId,
    outgoing: *Outgoing,
};

const Recovery = struct {
    connection_id: core_module.ConnectionId,
    correlation_id: WireIdentifier,
    keeper: Keeper,
    value: decode.Recover,
    /// When the request's `deadlineSeconds` ends on the recoveries' clock; a holding not yet
    /// claimed by then is declined.
    deadline_ns: ?u64,
    request: moves.Request,
    stage: Stage,
    /// The program once its handles passed the checks; the table takes it once it follows.
    entry: ?*Entry = null,
    answer: ?Answer = null,

    const Stage = enum { inspecting, claiming, following, declining, answering };
};

const Answer = union(enum) {
    recovered: struct { pid: u32, created: u64 },
    failed: host_response.FailureDetails,
};

pub const Recoveries = struct {
    allocator: std.mem.Allocator,
    pending: std.ArrayList(*Recovery) = .empty,
    timer: ?platform.monotonic.Timer,

    pub fn init(allocator: std.mem.Allocator) Recoveries {
        return .{ .allocator = allocator, .timer = platform.monotonic.Timer.start() catch null };
    }

    /// Recoveries not yet answered, each of which may add a running program.
    pub fn count(self: *const Recoveries) usize {
        return self.pending.items.len;
    }

    pub fn recovering(self: *const Recoveries, program_id: [16]u8) bool {
        for (self.pending.items) |recovery| if (std.mem.eql(u8, &recovery.value.program_id, &program_id)) return true;
        return false;
    }

    /// Inspects the holding through `keeper`'s link; the answer follows once the recovery ends.
    pub fn begin(
        self: *Recoveries,
        attachments: *keeper_link.Attachments,
        keeper: Keeper,
        connection_id: core_module.ConnectionId,
        correlation_id: WireIdentifier,
        value: decode.Recover,
    ) error{OutOfMemory}!void {
        try self.pending.ensureUnusedCapacity(self.allocator, 1);
        const recovery = try self.allocator.create(Recovery);
        recovery.* = .{
            .connection_id = connection_id,
            .correlation_id = correlation_id,
            .keeper = keeper,
            .value = value,
            .deadline_ns = if (self.timer) |*timer| timer.read() +| @as(u64, value.deadline_seconds) * std.time.ns_per_s else null,
            .request = .init(.{ .inspect_program = .{ .holding = value.holding, .mark = value.mark } }),
            .stage = .inspecting,
        };
        self.pending.appendAssumeCapacity(recovery);
        attachments.submit(keeper, &recovery.request);
    }

    /// Advances each recovery whose Keeper request completed. `entries` is the program table in
    /// ascending id order; a recovered program joins it once its threads run.
    pub fn advance(self: *Recoveries, attachments: *keeper_link.Attachments, entries: *std.ArrayList(*Entry)) void {
        for (self.pending.items) |recovery| self.step(recovery, attachments, entries);
    }

    fn step(self: *Recoveries, recovery: *Recovery, attachments: *keeper_link.Attachments, entries: *std.ArrayList(*Entry)) void {
        switch (recovery.stage) {
            .answering => return,
            .following => return self.follow(recovery, entries),
            .inspecting, .claiming, .declining => {},
        }
        const result = recovery.request.poll() orelse return;
        switch (recovery.stage) {
            .inspecting => switch (result) {
                .inspected_program => |inspected| self.claimChecked(recovery, inspected, attachments, entries),
                else => finish(recovery, .{ .failed = refusal.keeper(result) }),
            },
            .claiming => switch (result) {
                .done => {
                    recovery.stage = .following;
                    self.follow(recovery, entries);
                },
                else => {
                    // The claim did not apply, so no slot holds the program and the Keeper keeps it.
                    recovery.entry.?.discard();
                    recovery.entry = null;
                    finish(recovery, .{ .failed = refusal.keeper(result) });
                },
            },
            // The answer was settled before the decline, which only gives the holding back.
            .declining => recovery.stage = .answering,
            .following, .answering => unreachable,
        }
    }

    /// Checks the pulled handles, then claims the holding for a program built from them.
    fn claimChecked(
        self: *Recoveries,
        recovery: *Recovery,
        inspected_program: moves.InspectedProgram,
        attachments: *keeper_link.Attachments,
        entries: *std.ArrayList(*Entry),
    ) void {
        if (comptime !is_windows and !posix_keeper) unreachable;
        const duplicates = inspected_program.duplicates;
        defer attachments.allocator.free(inspected_program.label);
        const failed: ?host_response.FailureDetails = if (!std.mem.eql(u8, &inspected_program.program_id, &recovery.value.program_id))
            refusal.failed("identity-mismatch")
        else if (listed(entries, recovery.value.program_id))
            refusal.failed("identity-mismatch")
        else if (!handlesMatch(duplicates, inspected_program.process))
            refusal.failed("identity-mismatch")
        else if (self.expired(recovery))
            refusal.failed("deadline")
        else
            null;
        if (failed) |details| {
            duplicates.close();
            return decline(recovery, details, attachments);
        }
        const entry = Entry.recovered(
            self.allocator,
            recovery.value.program_id,
            inspected_program.label,
            inspected_program.process.pid,
            inspected_program.process.created,
            if (comptime is_windows)
                .{ .job = duplicates.job, .process = duplicates.process, .streams = duplicates.streams }
            else
                .{ .streams = duplicates.streams, .parented = duplicates.parented },
        ) catch {
            duplicates.close();
            return decline(recovery, refusal.exhausted, attachments);
        };
        recovery.entry = entry;
        recovery.stage = .claiming;
        recovery.request = .init(.{ .claim = .{ .gate = entry.gateOf(), .claim = .{
            .holding = recovery.value.holding,
            .mark = recovery.value.mark,
            .program = inspected_program.process,
        } } });
        attachments.submit(recovery.keeper, &recovery.request);
    }

    /// Whether what the Host received is the program the Keeper holds: on Windows, its process and
    /// a kill-on-close Job; on macOS, each captured stream's pipe read end and a program that runs
    /// with its start time or ended under the Keeper that parents it, which keeps it until its
    /// release.
    fn handlesMatch(duplicates: keeper_link.moves.InspectedProgram.Duplicates, process: keeper_link.moves.Process) bool {
        if (comptime is_windows) {
            const handles = platform.keeper.handles;
            return handles.programMatches(duplicates, .{ .pid = process.pid, .created = process.created }) and
                handles.killsOnClose(duplicates.job);
        }
        const native = platform.keeper.native;
        for (duplicates.streams) |stream| if (stream) |descriptor| if (!native.pipeEnd(descriptor, false)) return false;
        return (duplicates.parented and duplicates.ended) or native.processStart(process.pid) == process.created;
    }

    /// Starts the claimed program's threads and lists it; what cannot start yet retries on the
    /// next pass.
    fn follow(_: *Recoveries, recovery: *Recovery, entries: *std.ArrayList(*Entry)) void {
        const entry = recovery.entry.?;
        entries.ensureUnusedCapacity(entry.allocator, 1) catch return;
        entry.follow() catch return;
        var at: usize = 0;
        while (at < entries.items.len and std.mem.order(u8, &entries.items[at].id, &entry.id) == .lt) at += 1;
        entries.insertAssumeCapacity(at, entry);
        recovery.entry = null;
        finish(recovery, .{ .recovered = .{ .pid = entry.spawned.pid, .created = entry.spawned.created } });
    }

    fn expired(self: *Recoveries, recovery: *const Recovery) bool {
        const deadline = recovery.deadline_ns orelse return false;
        const timer = if (self.timer) |*timer| timer else return false;
        return timer.read() > deadline;
    }

    /// Answers each finished recovery to the session that asked; one whose session ended goes
    /// unanswered. An answer that does not fit waits for the next delivery.
    pub fn deliver(self: *Recoveries, epoch: identifiers.HostEpoch, session: ?Session) void {
        var index: usize = 0;
        while (index < self.pending.items.len) {
            const recovery = self.pending.items[index];
            if (recovery.stage != .answering) {
                index += 1;
                continue;
            }
            const current = session orelse {
                self.drop(index);
                continue;
            };
            if (current.connection_id != recovery.connection_id) {
                self.drop(index);
                continue;
            }
            answer(self.allocator, recovery, epoch, current.outgoing) catch {
                index += 1;
                continue;
            };
            self.drop(index);
        }
    }

    fn drop(self: *Recoveries, index: usize) void {
        const recovery = self.pending.orderedRemove(index);
        self.allocator.destroy(recovery);
    }

    /// Host teardown, after its dispatch loop ended: waits for each Keeper request still in flight,
    /// bounded by its link's reply deadline, then ends what a claim gave this Host and drops the
    /// rest, which its Keeper keeps.
    pub fn teardown(self: *Recoveries, attachments: *keeper_link.Attachments) void {
        for (self.pending.items) |recovery| {
            switch (recovery.stage) {
                .inspecting => switch (recovery.request.wait()) {
                    .inspected_program => |inspected_program| {
                        inspected_program.duplicates.close();
                        attachments.allocator.free(inspected_program.label);
                    },
                    else => {},
                },
                .claiming => if (recovery.request.wait() == .done) recovery.entry.?.end() else recovery.entry.?.discard(),
                .declining => _ = recovery.request.wait(),
                .following => recovery.entry.?.end(),
                .answering => {},
            }
            self.allocator.destroy(recovery);
        }
        self.pending.deinit(self.allocator);
        self.pending = .empty;
    }
};

/// Gives the holding back to the Keeper, which keeps the program running, and answers `details`
/// once the Keeper answered.
fn decline(recovery: *Recovery, details: host_response.FailureDetails, attachments: *keeper_link.Attachments) void {
    recovery.answer = .{ .failed = details };
    recovery.stage = .declining;
    recovery.request = .init(.{ .decline = .{ .gate = null, .marked = .{
        .holding = recovery.value.holding,
        .mark = recovery.value.mark,
    } } });
    attachments.submit(recovery.keeper, &recovery.request);
}

fn finish(recovery: *Recovery, value: Answer) void {
    recovery.answer = value;
    recovery.stage = .answering;
}

fn listed(entries: *const std.ArrayList(*Entry), program_id: [16]u8) bool {
    for (entries.items) |entry| if (std.mem.eql(u8, &entry.id, &program_id)) return true;
    return false;
}

fn answer(allocator: std.mem.Allocator, recovery: *Recovery, epoch: identifiers.HostEpoch, outgoing: *Outgoing) !void {
    const delivery = host_response.responseDelivery(recovery.correlation_id, epoch);
    switch (recovery.answer.?) {
        .recovered => |process| {
            const payload = try encode.recovered(allocator, recovery.value.program_id, process.pid, process.created);
            defer allocator.free(payload);
            _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.recover_program }, payload, delivery);
        },
        .failed => |details| try host_response.appendFailure(allocator, outgoing, details, delivery),
    }
}
