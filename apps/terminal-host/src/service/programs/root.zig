//! The `background-programs` capability of the Worker session: the programs the Host runs without a
//! terminal for its Worker, their requests, and `ProgramExited`. Programs outlive the session that
//! started them, so a Worker that replaces it lists them and collects their outcomes. Each attached
//! Keeper that takes programs holds every running one, so a program outlives its Host too, and a
//! recovery Host takes it back with `RecoverProgram`. Only the dispatch thread calls it.

const std = @import("std");
const builtin = @import("builtin");
const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const keeper_link = @import("terminal-host-keeper-link");
const host_outgoing = @import("../outgoing.zig");
const host_request_context = @import("../request_context.zig");
const host_response = @import("../response.zig");
const refusal = @import("../moves/refusal.zig");
const identifiers = @import("terminal-host-model").identifiers;
const entry_module = @import("entry.zig");
const recover_module = @import("recover.zig");

const Entry = entry_module.Entry;
const is_windows = builtin.os.tag == .windows;
const posix_keeper = builtin.os.tag == .macos or builtin.os.tag == .linux;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const decode = control.decode.program;
const encode = control.encode.program;

/// The Worker's own limit on running programs.
pub const max_running = 32;
/// Running and exited programs the Worker has not disposed.
pub const max_listed = 64;

pub const Session = recover_module.Session;

pub const Programs = struct {
    allocator: std.mem.Allocator,
    /// In ascending program id order.
    entries: std.ArrayList(*Entry) = .empty,
    recoveries: recover_module.Recoveries,
    negotiated: bool = false,

    pub fn init(allocator: std.mem.Allocator) Programs {
        return .{ .allocator = allocator, .recoveries = .init(allocator) };
    }

    /// Ends every program, each released from its Keepers as it ends, and frees the table; the
    /// Host's teardown calls it after its dispatch loop ended, while the Keeper links still run.
    pub fn teardown(self: *Programs, attachments: *keeper_link.Attachments) void {
        self.recoveries.teardown(attachments);
        for (self.entries.items) |entry| entry.end();
        self.entries.deinit(self.allocator);
        self.entries = .empty;
    }

    /// A Worker session was admitted; `negotiated` tells whether it listed `background-programs`.
    pub fn admit(self: *Programs, negotiated: bool) void {
        self.negotiated = negotiated;
    }

    pub fn forSession(self: *Programs) ?*Programs {
        return if (self.negotiated) self else null;
    }

    /// Programs still running or being recovered, which keep the Host from ending when idle or
    /// drained.
    pub fn running(self: *const Programs) usize {
        var count: usize = self.recoveries.count();
        for (self.entries.items) |entry| count += @intFromBool(entry.outcome() == null);
        return count;
    }

    pub fn handle(
        self: *Programs,
        context: *host_request_context.Context,
        operation: protocol.RequestOperation,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: WireIdentifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try context.requireControl(envelope, correlation_id, outgoing)) return;
        const delivery = host_response.responseDelivery(correlation_id, context.hostEpoch());
        const allocator = context.allocator;
        switch (operation) {
            .start_program => {
                var decoded = decode.decodeStart(allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "StartProgram payload is invalid");
                defer decoded.deinit();
                try self.start(context, decoded.value, outgoing, delivery);
            },
            .cancel_program => {
                var decoded = decode.decodeProgram(allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "CancelProgram payload is invalid");
                defer decoded.deinit();
                const entry = self.find(decoded.value) orelse return missing(context, outgoing, delivery);
                const payload = try encode.cancelled(allocator, entry.id, if (entry.cancel()) "cancelling" else "exited");
                defer allocator.free(payload);
                _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.cancel_program }, payload, delivery);
            },
            .read_program_output => {
                var decoded = decode.decodeReadOutput(allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "ReadProgramOutput payload is invalid");
                defer decoded.deinit();
                try self.readOutput(context, decoded.value, outgoing, delivery);
            },
            .dispose_program => {
                var decoded = decode.decodeProgram(allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "DisposeProgram payload is invalid");
                defer decoded.deinit();
                if (self.index(decoded.value)) |at| {
                    const entry = self.entries.items[at];
                    if (entry.outcome() == null) return failure(context, outgoing, delivery, .{
                        .code = "invalid-phase",
                        .message = "The program is still running",
                        .retryable = true,
                    });
                    _ = self.entries.orderedRemove(at);
                    entry.end();
                }
                const payload = try encode.programNamed(allocator, decoded.value);
                defer allocator.free(payload);
                _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.dispose_program }, payload, delivery);
            },
            .list_programs => {
                var decoded = decode.decodeList(allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "ListPrograms payload is invalid");
                defer decoded.deinit();
                try self.list(allocator, decoded.value, outgoing, delivery);
            },
            .recover_program => {
                var decoded = decode.decodeRecover(allocator, envelope.payload) catch |err|
                    return context.rejectControlDecode(outgoing, correlation_id, err, "RecoverProgram payload is invalid");
                defer decoded.deinit();
                try self.recover(context, connection_id, correlation_id, decoded.value, outgoing, delivery);
            },
            else => unreachable,
        }
    }

    /// Starts the recovery of a program the Host's one Keeper kept; it answers once it ends. The
    /// same recovery again answers the program it recovered.
    fn recover(
        self: *Programs,
        context: *host_request_context.Context,
        connection_id: core_module.ConnectionId,
        correlation_id: WireIdentifier,
        request: decode.Recover,
        outgoing: *Outgoing,
        delivery: host_outgoing.Delivery,
    ) !void {
        if (self.find(request.program_id)) |entry| {
            if (comptime is_windows or posix_keeper) if (entry.gateOf().knownId()) |known| if (std.mem.eql(u8, &known, &request.holding)) {
                const payload = try encode.recovered(context.allocator, entry.id, entry.spawned.pid, entry.spawned.created);
                defer context.allocator.free(payload);
                _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.recover_program }, payload, delivery);
                return;
            };
            return failure(context, outgoing, delivery, refusal.failed("identity-mismatch"));
        }
        if (self.recoveries.recovering(request.program_id)) return failure(context, outgoing, delivery, refusal.moving);
        if (context.terminal_admission == .closed) return failure(context, outgoing, delivery, .{
            .code = "host-draining",
            .message = "The Host no longer admits programs",
            .retryable = false,
        });
        if (self.running() >= max_running or self.entries.items.len + self.recoveries.count() >= max_listed)
            return failure(context, outgoing, delivery, .{
                .code = "program-limit",
                .message = "The Host runs or lists as many programs as it may",
                .retryable = true,
            });
        const keepers = context.keepers orelse return failure(context, outgoing, delivery, refusal.failed("no-keeper"));
        const keeper = keepers.attachments.sole() orelse return failure(context, outgoing, delivery, refusal.failed("no-keeper"));
        self.recoveries.begin(&keepers.attachments, keeper, connection_id, correlation_id, request) catch
            return failure(context, outgoing, delivery, refusal.exhausted);
    }

    fn start(
        self: *Programs,
        context: *host_request_context.Context,
        request: decode.Start,
        outgoing: *Outgoing,
        delivery: host_outgoing.Delivery,
    ) !void {
        const allocator = context.allocator;
        if (self.find(request.program_id)) |entry| return answerStarted(allocator, entry, true, outgoing, delivery);
        if (context.terminal_admission == .closed) return failure(context, outgoing, delivery, .{
            .code = "host-draining",
            .message = "The Host no longer admits programs",
            .retryable = false,
        });
        if (self.running() >= max_running or self.entries.items.len >= max_listed)
            return failure(context, outgoing, delivery, .{
                .code = "program-limit",
                .message = "The Host runs or lists as many programs as it may",
                .retryable = true,
            });
        try self.entries.ensureUnusedCapacity(self.allocator, 1);
        const environment = try allocator.alloc(platform.process.EnvironmentDelta, request.environment.len);
        defer allocator.free(environment);
        for (environment, request.environment) |*target, source| target.* = .{ .name = source.name, .value = source.value };
        // A macOS program starts through the newest attached Keeper that parents programs.
        var spawner = if (comptime posix_keeper) if (context.keepers) |keepers| keepers.attachments.programSpawner() else null else {};
        defer if (comptime posix_keeper) if (spawner) |*pinned| pinned.unpin();
        const entry = Entry.start(self.allocator, request.program_id, request.label, .{
            .executable = request.executable,
            .arguments = request.arguments,
            .raw_argument = request.raw_argument,
            .directory = request.directory,
            .environment = environment,
            .capture = request.capture,
            .keeper = if (comptime posix_keeper) if (spawner) |*pinned| pinned.hook() else null else {},
            .program_id = request.program_id,
            .label = request.label,
        }) catch |err| return failure(context, outgoing, delivery, switch (err) {
            error.OutOfMemory, error.SystemResources, error.ThreadQuotaExceeded, error.LockedMemoryLimitExceeded => .{
                .code = "resource-exhausted",
                .message = "The Host could not start the program now",
                .retryable = true,
            },
            error.NotFound => spawnFailed("not-found"),
            error.InvalidCommand => spawnFailed("invalid-command"),
            else => spawnFailed("failed"),
        });
        var at: usize = 0;
        while (at < self.entries.items.len and std.mem.order(u8, &self.entries.items[at].id, &entry.id) == .lt) at += 1;
        self.entries.insertAssumeCapacity(at, entry);
        try answerStarted(allocator, entry, false, outgoing, delivery);
    }

    fn readOutput(
        self: *Programs,
        context: *host_request_context.Context,
        request: decode.ReadOutput,
        outgoing: *Outgoing,
        delivery: host_outgoing.Delivery,
    ) !void {
        const entry = self.find(request.program_id) orelse return missing(context, outgoing, delivery);
        if (entry.outcome() == null) return failure(context, outgoing, delivery, .{
            .code = "invalid-phase",
            .message = "The program is still running",
            .retryable = true,
        });
        const stream: platform.program.Stream = @enumFromInt(@intFromEnum(request.stream));
        const bytes = entry.captured(stream) orelse "";
        if (request.offset > bytes.len) return failure(context, outgoing, delivery, .{
            .code = "invalid-field",
            .message = "The offset is past the stream's end",
            .retryable = false,
        });
        const chunk = bytes[request.offset..][0..@min(bytes.len - request.offset, encode.max_output_chunk_bytes)];
        const end = request.offset + chunk.len == bytes.len;
        const payload = try encode.output(context.allocator, entry.id, @tagName(request.stream), request.offset, chunk, end);
        defer context.allocator.free(payload);
        _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.read_program_output }, payload, delivery);
    }

    fn list(
        self: *Programs,
        allocator: std.mem.Allocator,
        after: ?[16]u8,
        outgoing: *Outgoing,
        delivery: host_outgoing.Delivery,
    ) !void {
        var page: [encode.max_list_page]encode.Summary = undefined;
        var count: usize = 0;
        var more = false;
        for (self.entries.items) |entry| {
            if (after) |bound| if (std.mem.order(u8, &entry.id, &bound) != .gt) continue;
            if (count == page.len) {
                more = true;
                break;
            }
            page[count] = entry.summary();
            count += 1;
        }
        const payload = try encode.list(allocator, page[0..count], more);
        defer allocator.free(payload);
        _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.list_programs }, payload, delivery);
    }

    /// Offers every running program to the attached Keepers, advances the recoveries, answers
    /// those that ended, and sends `ProgramExited` for each program that completed since a session
    /// last heard of it. Infallible: what does not fit stays for the next pass.
    pub fn deliver(self: *Programs, epoch: identifiers.HostEpoch, session: ?Session, attachments: *keeper_link.Attachments) void {
        self.offer(attachments);
        self.recoveries.advance(attachments, &self.entries);
        self.recoveries.deliver(epoch, session);
        const current = session orelse return;
        if (!self.negotiated) return;
        for (self.entries.items) |entry| {
            if (entry.announced or entry.outcome() == null) continue;
            const payload = encode.exited(self.allocator, entry.summary()) catch return;
            defer self.allocator.free(payload);
            _ = host_response.appendRequired(
                current.outgoing,
                MessageKind.program_exited,
                .control_json,
                payload,
                host_response.eventDelivery(epoch),
            ) catch return;
            entry.announced = true;
        }
    }

    /// Each attachment tries each program once; a Keeper without `programs` takes none.
    fn offer(self: *Programs, attachments: *keeper_link.Attachments) void {
        if (comptime !is_windows and !posix_keeper) return;
        if (!attachments.any()) return;
        for (self.entries.items) |entry| {
            if (entry.outcome() != null) continue;
            const facts: @FieldType(keeper_link.Program, "facts") = .{
                .process = .{ .pid = entry.spawned.pid, .created = entry.spawned.created },
                .program_id = entry.id,
                .label = entry.label,
            };
            attachments.offerProgram(entry.gateOf(), if (comptime is_windows) .{
                .job = entry.spawned.job,
                .process = entry.spawned.process,
                .streams = entry.spawned.streams,
                .facts = facts,
            } else .{ .streams = entry.spawned.streams, .facts = facts });
        }
    }

    fn index(self: *const Programs, id: [16]u8) ?usize {
        for (self.entries.items, 0..) |entry, at| if (std.mem.eql(u8, &entry.id, &id)) return at;
        return null;
    }

    fn find(self: *const Programs, id: [16]u8) ?*Entry {
        return if (self.index(id)) |at| self.entries.items[at] else null;
    }
};

fn answerStarted(allocator: std.mem.Allocator, entry: *Entry, existing: bool, outgoing: *Outgoing, delivery: host_outgoing.Delivery) !void {
    const payload = try encode.started(allocator, entry.id, entry.spawned.pid, entry.spawned.created, existing);
    defer allocator.free(payload);
    _ = try host_response.appendControlSuccess(outgoing, .{ .kind = MessageKind.start_program }, payload, delivery);
}

fn spawnFailed(reason: []const u8) host_response.FailureDetails {
    return .{
        .code = "spawn-failed",
        .message = "The Host could not start the program",
        .retryable = false,
        .reason = reason,
    };
}

fn missing(context: *host_request_context.Context, outgoing: *Outgoing, delivery: host_outgoing.Delivery) !void {
    return failure(context, outgoing, delivery, .{
        .code = "missing-resource",
        .message = "The Host lists no such program",
        .retryable = false,
    });
}

fn failure(
    context: *host_request_context.Context,
    outgoing: *Outgoing,
    delivery: host_outgoing.Delivery,
    details: host_response.FailureDetails,
) !void {
    try host_response.appendFailure(context.allocator, outgoing, details, delivery);
}
