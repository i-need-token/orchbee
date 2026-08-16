const std = @import("std");
const builtin = @import("builtin");
const protocol = @import("terminal-protocol");
const black_box_transport = @import("black_box_transport.zig");

const endpoint_startup_timeout_ns = 5 * std.time.ns_per_s;
const record_timeout_ns = 5 * std.time.ns_per_s;
const host_exit_timeout_ns = 10 * std.time.ns_per_s;
const creation_id_text = "31313131313131313131313131313131";
const attachment_id_text = "44444444444444444444444444444444";
const attachment_id = [_]u8{0x44} ** 16;

const TraceEvent = enum {
    host_spawned,
    ipc_connected,
    hello_accepted,
    terminal_created,
    terminal_started,
    attachment_committed,
    ipc_disconnected,
    ipc_reconnected,
    reconnect_accepted,
    creation_replayed,
    attachment_recommitted,
    input_accepted,
    terminal_exited,
    terminal_disposed,
    host_exited,
};

const expected_trace = [_]TraceEvent{
    .host_spawned,
    .ipc_connected,
    .hello_accepted,
    .terminal_created,
    .terminal_started,
    .attachment_committed,
    .ipc_disconnected,
    .ipc_reconnected,
    .reconnect_accepted,
    .creation_replayed,
    .attachment_recommitted,
    .input_accepted,
    .terminal_exited,
    .terminal_disposed,
    .host_exited,
};

const Trace = struct {
    cursor: usize = 0,

    fn observe(self: *Trace, event: TraceEvent) !void {
        if (self.cursor >= expected_trace.len or expected_trace[self.cursor] != event) {
            const expected = if (self.cursor < expected_trace.len)
                @tagName(expected_trace[self.cursor])
            else
                "end-of-trace";
            std.debug.print(
                "terminal Host black-box trace mismatch: expected {s}, observed {s}\n",
                .{ expected, @tagName(event) },
            );
            return error.TraceMismatch;
        }
        self.cursor += 1;
    }

    fn expectComplete(self: *const Trace) !void {
        if (self.cursor != expected_trace.len) return error.IncompleteTrace;
    }
};

const OwnedEnvelope = struct {
    allocator: std.mem.Allocator,
    bytes: []u8,
    summary: protocol.EnvelopeSummary,

    fn read(allocator: std.mem.Allocator, client: *black_box_transport.Client) !OwnedEnvelope {
        const bytes = try black_box_transport.readRecordBefore(
            allocator,
            client,
            record_timeout_ns,
        );
        errdefer allocator.free(bytes);
        return .{
            .allocator = allocator,
            .bytes = bytes,
            .summary = try protocol.validateTransportEnvelope(bytes),
        };
    }

    fn deinit(self: *OwnedEnvelope) void {
        self.allocator.free(self.bytes);
        self.* = undefined;
    }
};

const IdentityWire = struct {
    creationId: []const u8,
    hostEpoch: []const u8,
    terminalId: []const u8,
};

const HelloAcceptedWire = struct {
    hostEpoch: []const u8,
};

const TerminalExitedWire = struct {
    exit: struct {
        code: ?u32 = null,
        kind: []const u8,
        signal: ?u32 = null,
        value: ?u32 = null,
    },
    finalRevision: []const u8,
    identity: IdentityWire,
};

const Session = struct {
    allocator: std.mem.Allocator,
    client: *black_box_transport.Client,
    exit_seen: bool = false,
    host_epoch: [16]u8,
    started_seen: bool = false,
    terminal_id: ?[16]u8 = null,
    trace: *Trace,

    fn request(
        self: *Session,
        request_kind: u16,
        encoding: protocol.PayloadEncoding,
        payload: []const u8,
        correlation_byte: u8,
    ) !OwnedEnvelope {
        const correlation_id = [_]u8{correlation_byte} ** 16;
        const record = try protocol.encodeTransportEnvelope(
            self.allocator,
            request_kind,
            encoding,
            payload,
            correlation_id,
            self.host_epoch,
            false,
            false,
        );
        defer self.allocator.free(record);
        try self.client.writeAll(record);

        const contract = protocol.messageContract(request_kind) orelse
            return error.UnknownRequestKind;
        const request_contract = contract.request orelse return error.NotARequestKind;
        while (true) {
            var incoming = try OwnedEnvelope.read(self.allocator, self.client);
            if (incoming.summary.event) {
                try self.observeEvent(incoming.summary);
                incoming.deinit();
                continue;
            }
            errdefer incoming.deinit();
            if (!incoming.summary.response or
                incoming.summary.correlation_id == null or
                !std.mem.eql(u8, &incoming.summary.correlation_id.?, &correlation_id) or
                incoming.summary.host_epoch == null or
                !std.mem.eql(u8, &incoming.summary.host_epoch.?, &self.host_epoch))
            {
                return error.UnexpectedResponseEnvelope;
            }
            if (incoming.summary.kind == protocol.MessageKind.typed_failure) {
                std.debug.print(
                    "terminal Host rejected black-box request {d}: {s}\n",
                    .{ request_kind, incoming.summary.payload },
                );
                return error.HostRejectedRequest;
            }
            const response_contract = protocol.messageContract(
                request_contract.success_kind,
            ) orelse return error.UnknownResponseKind;
            const response_encoding = response_contract.response_encoding orelse
                return error.NotAResponseKind;
            if (incoming.summary.kind != request_contract.success_kind or
                incoming.summary.payload_encoding != @intFromEnum(response_encoding))
            {
                return error.UnexpectedResponseKind;
            }
            return incoming;
        }
    }

    fn awaitExit(self: *Session) !void {
        while (!self.exit_seen) {
            var incoming = try OwnedEnvelope.read(self.allocator, self.client);
            defer incoming.deinit();
            if (!incoming.summary.event) return error.UnexpectedResponseEnvelope;
            try self.observeEvent(incoming.summary);
        }
    }

    fn awaitStarted(self: *Session) !void {
        while (!self.started_seen) {
            var incoming = try OwnedEnvelope.read(self.allocator, self.client);
            defer incoming.deinit();
            if (!incoming.summary.event) return error.UnexpectedResponseEnvelope;
            try self.observeEvent(incoming.summary);
        }
    }

    fn observeEvent(self: *Session, envelope: protocol.EnvelopeSummary) !void {
        if (envelope.host_epoch == null or
            !std.mem.eql(u8, &envelope.host_epoch.?, &self.host_epoch))
        {
            return error.UnexpectedEventEpoch;
        }
        switch (envelope.kind) {
            protocol.MessageKind.terminal_started => {
                const identity = try std.json.parseFromSlice(
                    IdentityWire,
                    self.allocator,
                    envelope.payload,
                    .{ .ignore_unknown_fields = false },
                );
                defer identity.deinit();
                try self.expectIdentity(identity.value);
                if (self.started_seen) return error.DuplicateTerminalStarted;
                self.started_seen = true;
                try self.trace.observe(.terminal_started);
            },
            protocol.MessageKind.terminal_exited => {
                const notice = try std.json.parseFromSlice(
                    TerminalExitedWire,
                    self.allocator,
                    envelope.payload,
                    .{ .ignore_unknown_fields = false },
                );
                defer notice.deinit();
                try self.expectIdentity(notice.value.identity);
                if (!std.mem.eql(u8, notice.value.exit.kind, "exit-code") or
                    notice.value.exit.code != 0 or
                    notice.value.exit.signal != null or
                    notice.value.exit.value != null)
                {
                    return error.UnexpectedTerminalExit;
                }
                const final_revision = try std.fmt.parseUnsigned(
                    u64,
                    notice.value.finalRevision,
                    10,
                );
                if (final_revision == 0) return error.MissingPtyOutputRevision;
                if (self.exit_seen) return error.DuplicateTerminalExit;
                self.exit_seen = true;
                try self.trace.observe(.terminal_exited);
            },
            else => return error.UnexpectedHostEvent,
        }
    }

    fn expectIdentity(self: *const Session, identity: IdentityWire) !void {
        if (!std.mem.eql(u8, identity.creationId, creation_id_text)) {
            return error.UnexpectedCreationIdentity;
        }
        const event_epoch = try parseIdentifier(identity.hostEpoch);
        if (!std.mem.eql(u8, &event_epoch, &self.host_epoch)) {
            return error.UnexpectedEventEpoch;
        }
        const expected_terminal = self.terminal_id orelse return error.MissingTerminalIdentity;
        const event_terminal = try parseIdentifier(identity.terminalId);
        if (!std.mem.eql(u8, &event_terminal, &expected_terminal)) {
            return error.UnexpectedTerminalIdentity;
        }
    }
};

pub fn main() !void {
    var allocator_state: std.heap.DebugAllocator(.{}) = .init;
    defer if (allocator_state.deinit() == .leak) {
        @panic("terminal Host black-box test leaked memory");
    };
    const allocator = allocator_state.allocator();
    const arguments = try std.process.argsAlloc(allocator);
    defer std.process.argsFree(allocator, arguments);
    if (arguments.len != 2) return error.InvalidArguments;
    try runTrace(allocator, arguments[1]);
}

fn runTrace(allocator: std.mem.Allocator, host_executable: []const u8) !void {
    var runtime_directory = try black_box_transport.RuntimeDirectory.init(allocator);
    defer runtime_directory.deinit();
    var endpoint_bytes: [16]u8 = undefined;
    std.crypto.random.bytes(&endpoint_bytes);
    const endpoint_id = std.fmt.bytesToHex(endpoint_bytes, .lower);

    const host_arguments = [_][]const u8{
        host_executable,
        "serve",
        "--runtime-directory",
        runtime_directory.path(),
        "--endpoint-id",
        &endpoint_id,
    };
    var child = std.process.Child.init(&host_arguments, allocator);
    child.stdin_behavior = .Ignore;
    try child.spawn();
    var child_reaped = false;
    defer if (!child_reaped) {
        _ = child.kill() catch {
            _ = child.wait() catch {};
        };
    };

    var trace: Trace = .{};
    try trace.observe(.host_spawned);
    var client = try black_box_transport.connectBefore(
        allocator,
        runtime_directory.path(),
        &endpoint_id,
        endpoint_startup_timeout_ns,
    );
    var client_open = true;
    defer if (client_open) client.close();
    try trace.observe(.ipc_connected);

    const host_epoch = try handshake(allocator, &client);
    try trace.observe(.hello_accepted);
    var session: Session = .{
        .allocator = allocator,
        .client = &client,
        .host_epoch = host_epoch,
        .trace = &trace,
    };

    const shell = try shellCommand(allocator);
    defer shell.deinit(allocator);
    const create_payload = try createPayload(allocator, shell.executable, shell.arguments);
    defer allocator.free(create_payload);
    const terminal_id = created_identity: {
        var created = try session.request(
            protocol.MessageKind.create_terminal,
            .control_json,
            create_payload,
            0x51,
        );
        defer created.deinit();
        const identity = try std.json.parseFromSlice(
            IdentityWire,
            allocator,
            created.summary.payload,
            .{ .ignore_unknown_fields = false },
        );
        defer identity.deinit();
        const terminal_id = try parseIdentifier(identity.value.terminalId);
        const identity_epoch = try parseIdentifier(identity.value.hostEpoch);
        if (!std.mem.eql(u8, identity.value.creationId, creation_id_text) or
            !std.mem.eql(u8, &identity_epoch, &host_epoch))
        {
            return error.UnexpectedTerminalIdentity;
        }
        break :created_identity terminal_id;
    };
    session.terminal_id = terminal_id;
    try trace.observe(.terminal_created);
    try session.awaitStarted();

    const attach_payload = try std.fmt.allocPrint(
        allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":false,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{ attachment_id_text, identityText(terminal_id) },
    );
    defer allocator.free(attach_payload);
    {
        var attached = try session.request(
            protocol.MessageKind.attach_terminal,
            .control_json,
            attach_payload,
            0x52,
        );
        defer attached.deinit();
    }
    try trace.observe(.attachment_committed);

    client.close();
    client_open = false;
    try trace.observe(.ipc_disconnected);
    client = try black_box_transport.connectBefore(
        allocator,
        runtime_directory.path(),
        &endpoint_id,
        endpoint_startup_timeout_ns,
    );
    client_open = true;
    try trace.observe(.ipc_reconnected);
    const reconnect_epoch = try handshake(allocator, &client);
    if (!std.mem.eql(u8, &reconnect_epoch, &host_epoch)) {
        return error.HostEpochChangedAcrossReconnect;
    }
    try trace.observe(.reconnect_accepted);

    {
        var replayed = try session.request(
            protocol.MessageKind.create_terminal,
            .control_json,
            create_payload,
            0x53,
        );
        defer replayed.deinit();
        const replayed_identity = try std.json.parseFromSlice(
            IdentityWire,
            allocator,
            replayed.summary.payload,
            .{ .ignore_unknown_fields = false },
        );
        defer replayed_identity.deinit();
        const replayed_terminal = try parseIdentifier(replayed_identity.value.terminalId);
        const replayed_epoch = try parseIdentifier(replayed_identity.value.hostEpoch);
        if (!std.mem.eql(u8, replayed_identity.value.creationId, creation_id_text) or
            !std.mem.eql(u8, &replayed_terminal, &terminal_id) or
            !std.mem.eql(u8, &replayed_epoch, &host_epoch))
        {
            return error.CreationReplayChangedIdentity;
        }
    }
    try trace.observe(.creation_replayed);

    {
        var reattached = try session.request(
            protocol.MessageKind.attach_terminal,
            .control_json,
            attach_payload,
            0x54,
        );
        defer reattached.deinit();
    }
    try trace.observe(.attachment_recommitted);

    const input_text = if (builtin.os.tag == .windows)
        "orchbee-black-box\r\n"
    else
        "orchbee-black-box\n";
    const input_payload = try allocator.alloc(u8, attachment_id.len + input_text.len);
    defer allocator.free(input_payload);
    @memcpy(input_payload[0..attachment_id.len], &attachment_id);
    @memcpy(input_payload[attachment_id.len..], input_text);
    {
        var accepted = try session.request(
            protocol.MessageKind.write_input,
            .raw_bytes,
            input_payload,
            0x55,
        );
        defer accepted.deinit();
    }
    try trace.observe(.input_accepted);

    try session.awaitExit();
    const dispose_payload = try std.fmt.allocPrint(
        allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identityText(terminal_id)},
    );
    defer allocator.free(dispose_payload);
    {
        var disposed = try session.request(
            protocol.MessageKind.dispose_terminal,
            .control_json,
            dispose_payload,
            0x56,
        );
        defer disposed.deinit();
    }
    try trace.observe(.terminal_disposed);

    client.close();
    client_open = false;
    const term = try waitForHost(&child);
    child_reaped = true;
    if (term != .Exited or term.Exited != 0) return error.UnexpectedHostExit;
    try trace.observe(.host_exited);
    try trace.expectComplete();
    std.debug.print(
        "terminal Host black-box trace passed: spawn -> IPC -> create/start/attach -> disconnect/reconnect/replay/reattach -> PTY input/output -> exit -> dispose -> shutdown\n",
        .{},
    );
}

fn handshake(
    allocator: std.mem.Allocator,
    client: *black_box_transport.Client,
) ![16]u8 {
    const payload = try std.fmt.allocPrint(
        allocator,
        "{{\"clientId\":\"22222222222222222222222222222222\",\"frameBytes\":1048576,\"inputBytes\":2048,\"protocol\":{{\"major\":{d},\"minor\":{d},\"patch\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{
            protocol.current_protocol_major,
            protocol.current_protocol_minor,
            protocol.current_protocol_patch,
        },
    );
    defer allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        allocator,
        protocol.MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer allocator.free(record);
    try client.writeAll(record);
    var response = try OwnedEnvelope.read(allocator, client);
    defer response.deinit();
    if (!response.summary.response or response.summary.event or
        response.summary.kind != protocol.MessageKind.hello_accepted or
        response.summary.correlation_id != null or response.summary.host_epoch == null)
    {
        return error.UnexpectedHandshakeResponse;
    }
    const accepted = try std.json.parseFromSlice(
        HelloAcceptedWire,
        allocator,
        response.summary.payload,
        .{ .ignore_unknown_fields = true },
    );
    defer accepted.deinit();
    const payload_epoch = try parseIdentifier(accepted.value.hostEpoch);
    if (!std.mem.eql(u8, &payload_epoch, &response.summary.host_epoch.?)) {
        return error.UnexpectedHandshakeEpoch;
    }
    return payload_epoch;
}

const ShellCommand = struct {
    arguments: []const []const u8,
    executable: []const u8,
    owned_executable: ?[]u8,

    fn deinit(self: ShellCommand, allocator: std.mem.Allocator) void {
        if (self.owned_executable) |value| allocator.free(value);
    }
};

fn shellCommand(allocator: std.mem.Allocator) !ShellCommand {
    if (builtin.os.tag == .windows) {
        const executable = try std.process.getEnvVarOwned(allocator, "ComSpec");
        const arguments = &[_][]const u8{
            "/Q",
            "/D",
            "/V:ON",
            "/S",
            "/C",
            "set /p value=& if \"!value!\"==\"orchbee-black-box\" (<nul set /p =black-box-output& exit /b 0) else (exit /b 9)",
        };
        return .{
            .arguments = arguments,
            .executable = executable,
            .owned_executable = executable,
        };
    }
    return .{
        .arguments = &.{
            "-c",
            "IFS= read -r value; if [ \"$value\" = orchbee-black-box ]; then printf black-box-output; exit 0; else exit 9; fi",
        },
        .executable = "/bin/sh",
        .owned_executable = null,
    };
}

fn createPayload(
    allocator: std.mem.Allocator,
    executable: []const u8,
    arguments: []const []const u8,
) ![]u8 {
    const Environment = struct { name: []const u8, value: ?[]const u8 };
    const environment = [_]Environment{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    var writer: std.Io.Writer.Allocating = .init(allocator);
    defer writer.deinit();
    try std.json.Stringify.value(.{
        .creationId = creation_id_text,
        .launch = .{
            .arguments = arguments,
            .cellPixelHeight = 16,
            .cellPixelWidth = 8,
            .columns = 80,
            .configuration = @as(?struct { scrollbackBytes: usize }, null),
            .currentDirectory = @as(?[]const u8, null),
            .environment = &environment,
            .executable = executable,
            .rows = 24,
            .terminalProfile = .{
                .colorScheme = "dark",
                .defaultBackgroundRgb = 0,
                .defaultCursorRgb = @as(?u32, null),
                .defaultForegroundRgb = 0xffffff,
                .deviceAttributes = .{
                    .primaryConformanceLevel = 62,
                    .primaryFeatures = &features,
                    .secondaryDeviceType = 1,
                    .secondaryFirmwareVersion = 0,
                    .secondaryRomCartridge = 0,
                    .tertiaryUnitId = 0,
                },
                .enquiryResponse = "",
                .palette = &palette,
                .versionResponse = "Orchbee black-box test",
            },
        },
    }, .{}, &writer.writer);
    return writer.toOwnedSlice();
}

fn parseIdentifier(text: []const u8) ![16]u8 {
    if (text.len != 32) return error.InvalidIdentifier;
    var result: [16]u8 = undefined;
    for (&result, 0..) |*byte, index| {
        const high = try hexDigit(text[index * 2]);
        const low = try hexDigit(text[index * 2 + 1]);
        byte.* = high << 4 | low;
    }
    return result;
}

fn hexDigit(value: u8) !u8 {
    return switch (value) {
        '0'...'9' => value - '0',
        'a'...'f' => value - 'a' + 10,
        else => error.InvalidIdentifier,
    };
}

fn identityText(identifier: [16]u8) [32]u8 {
    return std.fmt.bytesToHex(identifier, .lower);
}

const Watchdog = struct {
    completed: *std.atomic.Value(bool),
    host_id: std.process.Child.Id,
    timed_out: *std.atomic.Value(bool),

    fn run(self: Watchdog) void {
        var elapsed: u64 = 0;
        const interval = 10 * std.time.ns_per_ms;
        while (elapsed < host_exit_timeout_ns) : (elapsed += interval) {
            if (self.completed.load(.acquire)) return;
            std.Thread.sleep(interval);
        }
        if (self.completed.load(.acquire)) return;
        self.timed_out.store(true, .release);
        if (builtin.os.tag == .windows) {
            std.os.windows.TerminateProcess(self.host_id, 1) catch {};
        } else {
            std.posix.kill(self.host_id, std.posix.SIG.KILL) catch {};
        }
    }
};

fn waitForHost(child: *std.process.Child) !std.process.Child.Term {
    var completed = std.atomic.Value(bool).init(false);
    var timed_out = std.atomic.Value(bool).init(false);
    const watchdog = try std.Thread.spawn(.{}, Watchdog.run, .{Watchdog{
        .completed = &completed,
        .host_id = child.id,
        .timed_out = &timed_out,
    }});
    const result = child.wait();
    completed.store(true, .release);
    watchdog.join();
    if (timed_out.load(.acquire)) return error.HostExitTimeout;
    return result;
}
