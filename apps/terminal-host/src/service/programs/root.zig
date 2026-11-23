//! The `background-programs` capability of the Worker session: the programs the Host runs without a
//! terminal for its Worker, their requests, and `ProgramExited`. Programs outlive the session that
//! started them, so a Worker that replaces it lists them and collects their outcomes. Only the
//! dispatch thread calls it.

const std = @import("std");
const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const host_outgoing = @import("../outgoing.zig");
const host_request_context = @import("../request_context.zig");
const host_response = @import("../response.zig");
const identifiers = @import("terminal-host-model").identifiers;
const entry_module = @import("entry.zig");

const Entry = entry_module.Entry;
const MessageKind = protocol.MessageKind;
const Outgoing = host_outgoing.Outgoing;
const WireIdentifier = identifiers.IdentifierBytes;
const decode = control.decode.program;
const encode = control.encode.program;

/// The Worker's own limit on running programs.
pub const max_running = 32;
/// Running and exited programs the Worker has not disposed.
pub const max_listed = 64;

pub const Session = struct {
    connection_id: core_module.ConnectionId,
    outgoing: *Outgoing,
};

pub const Programs = struct {
    allocator: std.mem.Allocator,
    /// In ascending program id order.
    entries: std.ArrayList(*Entry) = .empty,
    negotiated: bool = false,

    pub fn init(allocator: std.mem.Allocator) Programs {
        return .{ .allocator = allocator };
    }

    /// Ends every program and frees the table; the Host's teardown calls it.
    pub fn teardown(self: *Programs) void {
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

    /// Programs still running, which keep the Host from ending when idle or drained.
    pub fn running(self: *const Programs) usize {
        var count: usize = 0;
        for (self.entries.items) |entry| count += @intFromBool(entry.outcome() == null);
        return count;
    }

    pub fn handle(
        self: *Programs,
        context: *host_request_context.Context,
        operation: protocol.RequestOperation,
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
            else => unreachable,
        }
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
        const entry = Entry.start(self.allocator, request.program_id, request.label, .{
            .executable = request.executable,
            .arguments = request.arguments,
            .raw_argument = request.raw_argument,
            .directory = request.directory,
            .environment = environment,
            .capture = request.capture,
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

    /// Sends `ProgramExited` for each program that completed since a session last heard of it.
    /// Infallible: an event that does not fit stays for the next pass.
    pub fn deliver(self: *Programs, epoch: identifiers.HostEpoch, session: ?Session) void {
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
