const std = @import("std");

const projection_interval_ns = 16 * std.time.ns_per_ms;
const max_control_json_bytes = 64 * 1024;
const max_inventory_page_terminals = 128;
const max_schedule_scan_per_poll = 64;
const max_scheduled_attachments_per_poll = 8;

const core_module = @import("host_core.zig");
const protocol = @import("terminal-protocol");
const pty_process = @import("pty_process.zig");
const terminal_history = @import("terminal_history.zig");
const terminal_runtime = @import("terminal_runtime.zig");

const MessageKind = struct {
    const hello: u16 = 1;
    const hello_accepted: u16 = 2;
    const protocol_rejected: u16 = 3;
    const ping: u16 = 4;
    const typed_failure: u16 = 5;
    const list_terminals: u16 = 10;
    const create_terminal: u16 = 11;
    const terminate_terminal: u16 = 12;
    const dispose_terminal: u16 = 13;
    const terminal_started: u16 = 14;
    const terminal_exited: u16 = 15;
    const force_terminate_terminal: u16 = 18;
    const attach_terminal: u16 = 20;
    const detach_terminal: u16 = 21;
    const attachment_capabilities: u16 = 22;
    const write_input: u16 = 30;
    const input_accepted: u16 = 31;
    const resize_terminal: u16 = 32;
    const resize_accepted: u16 = 33;
    const key_input: u16 = 34;
    const text_input: u16 = 35;
    const mouse_input: u16 = 36;
    const focus_input: u16 = 37;
    const paste_input: u16 = 38;
    const snapshot_frame: u16 = 41;
    const delta_frame: u16 = 42;
    const frame_applied: u16 = 43;
    const need_snapshot: u16 = 44;
    const read_rows: u16 = 45;
    const rows_snapshot: u16 = 46;
    const set_projection_demand: u16 = 47;
    const projection_demand_accepted: u16 = 48;
    const projection_blocked: u16 = 49;
    const rows_applied: u16 = 50;
    const set_history_demand: u16 = 51;
    const history_demand_accepted: u16 = 52;
    const history_changed: u16 = 53;
    const history_observed: u16 = 54;
    const selection_gesture: u16 = 60;
    const selection_command: u16 = 61;
    const selection_changed: u16 = 62;
    const selection_export: u16 = 63;
    const selection_export_started: u16 = 64;
    const selection_export_chunk: u16 = 65;
    const selection_export_chunk_applied: u16 = 66;
    const selection_export_finished: u16 = 67;
    const selection_export_cancel: u16 = 68;
};

const ProtocolRangeWire = struct {
    major: u16,
    maximumMinor: u16,
    minimumMinor: u16,
};

const HelloWire = struct {
    clientId: []const u8,
    frameBytes: usize,
    inputBytes: usize,
    protocol: ProtocolRangeWire,
    rowWindowBytes: usize,
    supportedOptionalFeatures: []const []const u8,
};

const EnvironmentWire = struct {
    name: []const u8,
    value: ?[]const u8,
};

const LaunchWire = struct {
    arguments: []const []const u8,
    cellPixelHeight: ?u16 = null,
    cellPixelWidth: ?u16 = null,
    columns: u16,
    currentDirectory: ?[]const u8,
    environment: []const EnvironmentWire,
    executable: []const u8,
    rows: u16,
    terminalProfile: ?TerminalProfileWire = null,
};

const DeviceAttributesWire = struct {
    primaryConformanceLevel: u16,
    primaryFeatures: []const u16,
    secondaryDeviceType: u16,
    secondaryFirmwareVersion: u16,
    secondaryRomCartridge: u16,
    tertiaryUnitId: u32,
};

const TerminalProfileWire = struct {
    colorScheme: []const u8,
    defaultBackgroundRgb: u32,
    defaultCursorRgb: ?u32,
    defaultForegroundRgb: u32,
    deviceAttributes: DeviceAttributesWire,
    enquiryResponse: []const u8,
    palette: []const u32,
    versionResponse: []const u8,
};

const CreateWire = struct {
    creationId: []const u8,
    launch: LaunchWire,
};

const InventoryCursorWire = struct {
    inventoryGeneration: []const u8,
    offset: []const u8,
};

const ListTerminalsV1_3Wire = struct {
    cursor: ?InventoryCursorWire,
    maximumTerminals: usize,
};

const TerminalTargetWire = struct { terminalId: []const u8 };

const CapabilitiesWire = struct {
    input: bool,
    read: bool,
    resize: bool,
};

const AttachWire = struct {
    attachmentId: []const u8,
    capabilities: CapabilitiesWire,
    requestResizeLease: bool,
    terminalId: []const u8,
};

const DetachWire = struct { attachmentId: []const u8 };

const ResizeWire = struct {
    attachmentId: []const u8,
    cellPixelHeight: ?u16 = null,
    cellPixelWidth: ?u16 = null,
    columns: u16,
    rows: u16,
};

const FrameAppliedWire = struct {
    attachmentId: []const u8,
    generation: []const u8,
    revision: []const u8,
    sequence: []const u8,
};

const NeedSnapshotWire = struct {
    attachmentId: []const u8,
    reason: []const u8,
    sequence: []const u8,
};

const SetProjectionDemandWire = struct {
    attachmentId: []const u8,
    demand: []const u8,
};

const RowOriginWire = struct {
    delta: ?i32 = null,
    kind: []const u8,
    offset: ?u32 = null,
    row: ?[]const u8 = null,
    rowsBefore: ?u32 = null,
};

const ReadRowsWire = struct {
    attachmentId: []const u8,
    detail: []const u8,
    generation: []const u8,
    maximumBytes: usize,
    origin: RowOriginWire,
    purpose: []const u8,
    requestSequence: []const u8,
    revision: []const u8,
    rowCount: u16,
};

const RowsAppliedWire = struct {
    attachmentId: []const u8,
    generation: []const u8,
    requestSequence: []const u8,
    revision: []const u8,
};

const SetHistoryDemandWire = struct {
    attachmentId: []const u8,
    demand: []const u8,
};

const HistoryObservedWire = struct {
    attachmentId: []const u8,
    generation: []const u8,
    revision: []const u8,
};

const SelectionPointWire = struct {
    column: u16,
    row: []const u8,
};

const SelectionGestureWire = struct {
    action: []const u8,
    attachmentId: []const u8,
    clickCount: u2,
    generation: []const u8,
    point: SelectionPointWire,
    rectangle: bool,
    revision: []const u8,
};

const SelectionCommandWire = struct {
    attachmentId: []const u8,
    command: []const u8,
    generation: []const u8,
    point: ?SelectionPointWire = null,
    revision: []const u8,
};

const SelectionExportWire = struct {
    attachmentId: []const u8,
    exportSequence: []const u8,
    format: []const u8,
};

const SelectionExportChunkAppliedWire = struct {
    attachmentId: []const u8,
    chunkSequence: u32,
    exportSequence: []const u8,
};

const SelectionExportCancelWire = struct {
    attachmentId: []const u8,
    exportSequence: []const u8,
    reason: []const u8,
};

const SemanticInput = union(enum) {
    focus: protocol.FocusInputSummary,
    key: protocol.KeyInputSummary,
    mouse: protocol.MouseInputSummary,
    paste: protocol.TextInputSummary,
    text: protocol.TextInputSummary,
};

pub const ConnectionState = struct {
    client_id: ?core_module.Identifier = null,
    close_after_response: bool = false,
    connection_id: ?core_module.ConnectionId = null,
    frame_bytes: usize = 0,
    input_bytes: usize = 0,
    protocol_minor: u16 = 0,
    row_window_bytes: usize = 0,
};

const RuntimeEntry = struct {
    next_projection_at_ns: i128 = 0,
    runtime: *terminal_runtime.TerminalRuntime,
};

pub const Outgoing = struct {
    allocator: std.mem.Allocator,
    protocol_minor: u16 = 0,
    records: std.ArrayList([]u8) = .empty,

    pub fn init(allocator: std.mem.Allocator) Outgoing {
        return .{ .allocator = allocator };
    }

    pub fn deinit(self: *Outgoing) void {
        for (self.records.items) |record| self.allocator.free(record);
        self.records.deinit(self.allocator);
        self.* = undefined;
    }

    fn add(
        self: *Outgoing,
        kind: u16,
        encoding: protocol.PayloadEncoding,
        payload: []const u8,
        correlation_id: ?core_module.Identifier,
        host_epoch: ?core_module.Identifier,
        response: bool,
        event: bool,
    ) !void {
        const record = try protocol.encodeTransportEnvelopeVersion(
            self.allocator,
            self.protocol_minor,
            kind,
            encoding,
            payload,
            correlation_id,
            host_epoch,
            response,
            event,
        );
        errdefer self.allocator.free(record);
        try self.records.append(self.allocator, record);
    }
};

pub const HostService = struct {
    allocator: std.mem.Allocator,
    core: core_module.HostCore,
    interaction_schedule_index: usize = 0,
    next_connection_id: core_module.ConnectionId = 1,
    projection_schedule_index: usize = 0,
    runtimes: std.AutoArrayHashMap(core_module.Identifier, RuntimeEntry),
    trace_interactions: bool = false,

    pub fn init(allocator: std.mem.Allocator) HostService {
        return .{
            .allocator = allocator,
            .core = .init(allocator, randomIdentifier()),
            .runtimes = .init(allocator),
        };
    }

    pub fn deinit(self: *HostService) void {
        for (self.runtimes.values()) |entry| entry.runtime.terminate() catch {};
        while (self.runtimes.count() > 0) {
            const terminal_id = self.runtimes.keys()[0];
            const entry = self.runtimes.fetchOrderedRemove(terminal_id).?.value;
            entry.runtime.destroy();
        }
        self.runtimes.deinit();
        self.core.deinit();
        self.* = undefined;
    }

    pub fn disconnect(self: *HostService, connection: *ConnectionState) void {
        if (connection.connection_id) |connection_id| {
            for (self.core.attachments.values()) |attachment| {
                if (attachment.connection_id == connection_id) {
                    if (self.runtimeFor(attachment.terminal_id)) |runtime| {
                        runtime.detachHistory(attachment.attachment_id);
                    }
                }
            }
            _ = self.core.disconnect(connection_id);
            for (self.core.terminals.keys()) |terminal_id| self.releaseProjectionIfIdle(terminal_id);
        }
        connection.* = .{};
    }

    pub fn canStopWhenIdle(self: *const HostService) bool {
        return self.core.connectionCount() == 0 and
            self.core.terminalCount() == 0 and
            self.runtimes.count() == 0;
    }

    pub fn pollEvents(self: *HostService, connection: *const ConnectionState) !Outgoing {
        var outgoing = Outgoing.init(self.allocator);
        outgoing.protocol_minor = connection.protocol_minor;
        errdefer outgoing.deinit();
        try self.synchronizeRuntimes(&outgoing, connection.connection_id != null);
        try self.scheduleInteraction(connection, &outgoing);
        try self.scheduleProjection(connection, &outgoing);
        return outgoing;
    }

    pub fn handle(
        self: *HostService,
        connection: *ConnectionState,
        record: []const u8,
    ) !Outgoing {
        var outgoing = Outgoing.init(self.allocator);
        errdefer outgoing.deinit();

        const envelope = protocol.validateTransportEnvelope(record) catch return error.InvalidTransportRecord;
        if (envelope.protocol_major == 1 and envelope.protocol_minor <= 4) {
            outgoing.protocol_minor = envelope.protocol_minor;
        }
        try self.synchronizeRuntimes(&outgoing, connection.connection_id != null);
        if (envelope.event or envelope.response) {
            try self.addFailure(&outgoing, envelope.correlation_id, "invalid-envelope-role", "client record must be a request", false);
            return outgoing;
        }
        if (envelope.protocol_major != 1 or envelope.protocol_minor > 4) {
            try self.addFailure(&outgoing, envelope.correlation_id, "unsupported-version", "unsupported envelope protocol version", false);
            return outgoing;
        }

        if (connection.client_id == null) {
            try self.handleHello(connection, envelope, &outgoing);
            return outgoing;
        }
        outgoing.protocol_minor = connection.protocol_minor;
        if (envelope.protocol_minor != connection.protocol_minor) {
            try self.addFailure(&outgoing, envelope.correlation_id, "unsupported-version", "envelope version differs from the negotiated protocol", false);
            return outgoing;
        }
        if (envelope.kind == MessageKind.hello) {
            try self.addFailure(&outgoing, envelope.correlation_id, "repeated-hello", "Hello is allowed exactly once per connection", false);
            return outgoing;
        }
        const correlation_id = envelope.correlation_id orelse {
            try self.addFailure(&outgoing, null, "missing-correlation", "post-handshake requests require CorrelationId", false);
            return outgoing;
        };
        if (envelope.host_epoch == null or !identifierEqual(envelope.host_epoch.?, self.core.epoch)) {
            try self.addFailure(&outgoing, correlation_id, "stale-epoch", "request HostEpoch is not current", false);
            return outgoing;
        }
        if (envelope.optional_message and !protocol.isKnownMessageKind(envelope.kind)) return outgoing;

        switch (envelope.kind) {
            MessageKind.ping => try self.handlePing(envelope, correlation_id, &outgoing),
            MessageKind.list_terminals => try self.handleList(connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.create_terminal => try self.handleCreate(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.terminate_terminal => try self.handleTerminate(connection.connection_id.?, envelope, correlation_id, &outgoing),
            MessageKind.force_terminate_terminal => try self.handleForceTerminate(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.dispose_terminal => try self.handleDispose(connection.connection_id.?, envelope, correlation_id, &outgoing),
            MessageKind.attach_terminal => try self.handleAttach(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.detach_terminal => try self.handleDetach(connection.connection_id.?, envelope, correlation_id, &outgoing),
            MessageKind.write_input => try self.handleInput(connection.connection_id.?, connection.input_bytes, envelope, correlation_id, &outgoing),
            MessageKind.key_input,
            MessageKind.text_input,
            MessageKind.mouse_input,
            MessageKind.focus_input,
            MessageKind.paste_input,
            => try self.handleSemanticInput(connection.connection_id.?, connection.protocol_minor, connection.input_bytes, envelope, correlation_id, &outgoing),
            MessageKind.resize_terminal => try self.handleResize(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.frame_applied => try self.handleFrameApplied(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.need_snapshot => try self.handleNeedSnapshot(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.set_projection_demand => try self.handleProjectionDemand(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.read_rows => try self.handleReadRows(connection.connection_id.?, connection.protocol_minor, connection.row_window_bytes, envelope, correlation_id, &outgoing),
            MessageKind.rows_applied => try self.handleRowsApplied(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.set_history_demand => try self.handleHistoryDemand(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.history_observed => try self.handleHistoryObserved(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.selection_gesture => try self.handleSelectionGesture(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.selection_command => try self.handleSelectionCommand(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.selection_export => try self.handleSelectionExport(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.selection_export_chunk_applied => try self.handleSelectionExportChunkApplied(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            MessageKind.selection_export_cancel => try self.handleSelectionExportCancel(connection.connection_id.?, connection.protocol_minor, envelope, correlation_id, &outgoing),
            else => try self.addFailure(&outgoing, correlation_id, "unsupported-in-slice", "message is not implemented by this Host slice", false),
        }
        try self.scheduleInteraction(connection, &outgoing);
        try self.scheduleProjection(connection, &outgoing);
        return outgoing;
    }

    fn handleHello(
        self: *HostService,
        connection: *ConnectionState,
        envelope: protocol.EnvelopeSummary,
        outgoing: *Outgoing,
    ) !void {
        if (envelope.kind != MessageKind.hello or envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.control_json) or envelope.host_epoch != null) {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "handshake-required", "first request must be Hello without HostEpoch", false);
            return;
        }
        const parsed = std.json.parseFromSlice(HelloWire, self.allocator, envelope.payload, .{}) catch {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "invalid-field", "Hello control payload is invalid", false);
            return;
        };
        defer parsed.deinit();
        const hello = parsed.value;
        const client_id = parseIdentifier(hello.clientId) catch {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "invalid-identifier", "ClientId is invalid", false);
            return;
        };
        if (!validHello(hello)) {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "invalid-field", "Hello limits or features are invalid", false);
            return;
        }
        if (hello.protocol.major != 1 or hello.protocol.minimumMinor > 4) {
            const payload = try std.fmt.allocPrint(
                self.allocator,
                "{{\"clientRange\":{{\"major\":{d},\"maximumMinor\":{d},\"minimumMinor\":{d}}},\"hostRange\":{{\"major\":1,\"maximumMinor\":4,\"minimumMinor\":0}},\"reason\":\"{s}\"}}",
                .{
                    hello.protocol.major,
                    hello.protocol.maximumMinor,
                    hello.protocol.minimumMinor,
                    if (hello.protocol.major != 1) "major-mismatch" else "minor-range-mismatch",
                },
            );
            defer self.allocator.free(payload);
            try outgoing.add(MessageKind.protocol_rejected, .control_json, payload, envelope.correlation_id, null, true, false);
            connection.close_after_response = true;
            return;
        }
        const chosen_minor: u16 = @min(hello.protocol.maximumMinor, 4);
        outgoing.protocol_minor = chosen_minor;
        const connection_id = self.takeConnectionId() catch {
            try self.addHandshakeFailure(outgoing, envelope.correlation_id, "resource-exhausted", "Host connection identity space is exhausted", false);
            connection.close_after_response = true;
            return;
        };
        _ = self.core.connect(connection_id, client_id) catch |err| {
            try self.addHandshakeFailure(
                outgoing,
                envelope.correlation_id,
                coreError(err).code,
                coreError(err).message,
                coreError(err).retryable,
            );
            return;
        };
        connection.* = .{
            .client_id = client_id,
            .connection_id = connection_id,
            .frame_bytes = @min(hello.frameBytes, 8 * 1024 * 1024),
            .input_bytes = @min(hello.inputBytes, 1024 * 1024),
            .protocol_minor = chosen_minor,
            .row_window_bytes = @min(hello.rowWindowBytes, 2 * 1024 * 1024),
        };
        var epoch_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"frameBytes\":{d},\"hostEpoch\":\"{s}\",\"inputBytes\":{d},\"protocol\":{{\"major\":1,\"minor\":{d}}},\"rowWindowBytes\":{d},\"supportedOptionalFeatures\":[]}}",
            .{
                connection.frame_bytes,
                identifierText(self.core.epoch, &epoch_buffer),
                connection.input_bytes,
                chosen_minor,
                connection.row_window_bytes,
            },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.hello_accepted, .control_json, payload, envelope.correlation_id, self.core.epoch, true, false);
    }

    fn takeConnectionId(self: *HostService) error{ConnectionIdentityExhausted}!core_module.ConnectionId {
        if (self.next_connection_id == std.math.maxInt(core_module.ConnectionId)) {
            return error.ConnectionIdentityExhausted;
        }
        const connection_id = self.next_connection_id;
        self.next_connection_id += 1;
        return connection_id;
    }

    fn handlePing(
        self: *HostService,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireEmptyControl(envelope, correlation_id, outgoing)) return;
        try outgoing.add(MessageKind.ping, .control_json, "{}", correlation_id, self.core.epoch, true, false);
    }

    fn handleList(
        self: *HostService,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (protocol_minor < 3) {
            if (!try self.requireEmptyControl(envelope, correlation_id, outgoing)) return;
            if (self.core.terminalCount() > 1) {
                try self.addFailure(
                    outgoing,
                    correlation_id,
                    "protocol-upgrade-required",
                    "protocol 1.3 is required to enumerate multiple terminals",
                    false,
                );
                return;
            }
            const payload = try self.terminalListJsonLegacy();
            defer self.allocator.free(payload);
            try outgoing.add(MessageKind.list_terminals, .control_json, payload, correlation_id, self.core.epoch, true, false);
            return;
        }

        const parsed = try self.parseControl(ListTerminalsV1_3Wire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        if (parsed.value.maximumTerminals == 0 or
            parsed.value.maximumTerminals > max_inventory_page_terminals)
        {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "inventory page size is invalid", false);
            return;
        }
        var offset: usize = 0;
        if (parsed.value.cursor) |cursor| {
            const generation = parseCanonicalU64(cursor.inventoryGeneration) catch {
                try self.addFailure(outgoing, correlation_id, "invalid-field", "inventory generation is not canonical uint64 text", false);
                return;
            };
            if (generation != self.core.inventory_generation) {
                try self.addFailure(outgoing, correlation_id, "stale-inventory-cursor", "terminal inventory membership changed", true);
                return;
            }
            const raw_offset = parseCanonicalU64(cursor.offset) catch {
                try self.addFailure(outgoing, correlation_id, "invalid-field", "inventory offset is not canonical uint64 text", false);
                return;
            };
            offset = std.math.cast(usize, raw_offset) orelse {
                try self.addFailure(outgoing, correlation_id, "invalid-field", "inventory offset exceeds the platform range", false);
                return;
            };
        }
        if (offset > self.core.terminalCount()) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "inventory offset exceeds terminal count", false);
            return;
        }
        const payload = try self.terminalListJsonV1_3(offset, parsed.value.maximumTerminals);
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.list_terminals, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleCreate(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireControl(envelope, correlation_id, outgoing)) return;
        const parsed = std.json.parseFromSlice(CreateWire, self.allocator, envelope.payload, .{}) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "CreateTerminal payload is invalid", false);
            return;
        };
        defer parsed.deinit();
        const request = parsed.value;
        const creation_id = parseIdentifier(request.creationId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "CreationId is invalid", false);
            return;
        };
        if (!validLaunch(request.launch, protocol_minor)) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "terminal launch is invalid", false);
            return;
        }
        const digest = launchDigest(self.allocator, request.launch) catch {
            try self.addFailure(outgoing, correlation_id, "resource-exhausted", "Host could not inspect the terminal launch", true);
            return;
        };

        if (self.core.resolveCreation(connection_id, creation_id, digest) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        }) |terminal_id| {
            const terminal = self.core.findTerminal(terminal_id) orelse unreachable;
            const payload = try self.identityJson(terminal.creation_id, terminal_id);
            defer self.allocator.free(payload);
            try outgoing.add(MessageKind.create_terminal, .control_json, payload, correlation_id, self.core.epoch, true, false);
            return;
        }

        if (protocol_minor < 3 and self.core.terminalCount() != 0) {
            try self.addFailure(
                outgoing,
                correlation_id,
                "protocol-upgrade-required",
                "protocol 1.3 is required to create another terminal",
                false,
            );
            return;
        }

        const environment = self.allocator.alloc(pty_process.EnvironmentDelta, request.launch.environment.len) catch {
            try self.addFailure(outgoing, correlation_id, "resource-exhausted", "Host could not retain the terminal environment", true);
            return;
        };
        defer self.allocator.free(environment);
        for (request.launch.environment, 0..) |entry, index| {
            environment[index] = .{ .name = entry.name, .value = entry.value };
        }
        const cell_pixel_height = request.launch.cellPixelHeight orelse 1;
        const cell_pixel_width = request.launch.cellPixelWidth orelse 1;
        var runtime_profile: ?terminal_runtime.TerminalProfile = null;
        if (request.launch.terminalProfile) |profile| {
            runtime_profile = .{
                .color_scheme = if (std.mem.eql(u8, profile.colorScheme, "dark")) .dark else .light,
                .default_background_rgb = profile.defaultBackgroundRgb,
                .default_cursor_rgb = profile.defaultCursorRgb,
                .default_foreground_rgb = profile.defaultForegroundRgb,
                .device_primary_conformance_level = profile.deviceAttributes.primaryConformanceLevel,
                .device_primary_features = profile.deviceAttributes.primaryFeatures,
                .device_secondary_type = profile.deviceAttributes.secondaryDeviceType,
                .device_secondary_firmware_version = profile.deviceAttributes.secondaryFirmwareVersion,
                .device_secondary_rom_cartridge = profile.deviceAttributes.secondaryRomCartridge,
                .device_tertiary_unit_id = profile.deviceAttributes.tertiaryUnitId,
                .enquiry_response = profile.enquiryResponse,
                .palette = profile.palette,
                .version_response = profile.versionResponse,
            };
        }
        self.core.reserveTerminal() catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        self.runtimes.ensureUnusedCapacity(1) catch {
            try self.addFailure(outgoing, correlation_id, "resource-exhausted", "Host could not reserve terminal runtime state", true);
            return;
        };

        var terminal_id = randomIdentifier();
        while (self.core.findTerminal(terminal_id) != null) terminal_id = randomIdentifier();
        const runtime = terminal_runtime.TerminalRuntime.createWithOptions(self.allocator, .{
            .arguments = request.launch.arguments,
            .cell_pixel_height = cell_pixel_height,
            .cell_pixel_width = cell_pixel_width,
            .columns = request.launch.columns,
            .current_directory = request.launch.currentDirectory,
            .environment = environment,
            .executable = request.launch.executable,
            .rows = request.launch.rows,
        }, .{
            .cell_pixel_height = cell_pixel_height,
            .cell_pixel_width = cell_pixel_width,
            .profile = runtime_profile,
        }) catch {
            try self.addFailure(outgoing, correlation_id, "spawn-failed", "PTY or child creation failed", true);
            return;
        };
        self.core.commitTerminal(
            connection_id,
            creation_id,
            terminal_id,
            digest,
            request.launch.columns,
            request.launch.rows,
        ) catch |err| {
            runtime.terminate() catch {};
            runtime.destroy();
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        self.runtimes.putAssumeCapacityNoClobber(terminal_id, .{ .runtime = runtime });
        self.core.synchronizeGeometry(
            terminal_id,
            request.launch.columns,
            request.launch.rows,
            cell_pixel_width,
            cell_pixel_height,
            0,
        ) catch unreachable;
        _ = self.core.spawnSucceeded(terminal_id) catch unreachable;

        const payload = try self.identityJson(creation_id, terminal_id);
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.create_terminal, .control_json, payload, correlation_id, self.core.epoch, true, false);
        try outgoing.add(MessageKind.terminal_started, .control_json, payload, null, self.core.epoch, false, true);
    }

    fn handleTerminate(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        const target = try self.parseControl(TerminalTargetWire, envelope, correlation_id, outgoing) orelse return;
        defer target.deinit();
        const terminal_id = parseIdentifier(target.value.terminalId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "TerminalId is invalid", false);
            return;
        };
        const outcome = self.core.requestTerminate(connection_id, terminal_id) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        if (outcome == .applied) {
            const runtime = self.runtimeFor(terminal_id) orelse {
                self.core.findTerminal(terminal_id).?.terminate_requested = false;
                try self.addFailure(outgoing, correlation_id, "missing-resource", "terminal runtime is missing", false);
                return;
            };
            runtime.terminate() catch {
                self.core.findTerminal(terminal_id).?.terminate_requested = false;
                try self.addFailure(outgoing, correlation_id, "termination-failed", "platform termination request failed", true);
                return;
            };
        }
        const payload = try terminalAcknowledgedJson(self.allocator, terminal_id);
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.terminate_terminal, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleForceTerminate(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_4(protocol_minor, correlation_id, outgoing)) return;
        const target = try self.parseControl(TerminalTargetWire, envelope, correlation_id, outgoing) orelse return;
        defer target.deinit();
        const terminal_id = parseIdentifier(target.value.terminalId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "TerminalId is invalid", false);
            return;
        };
        const outcome = self.core.requestForceTerminate(connection_id, terminal_id) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        if (outcome == .applied) {
            const runtime = self.runtimeFor(terminal_id) orelse {
                self.core.findTerminal(terminal_id).?.force_terminate_requested = false;
                try self.addFailure(outgoing, correlation_id, "missing-resource", "terminal runtime is missing", false);
                return;
            };
            runtime.forceTerminate() catch {
                self.core.findTerminal(terminal_id).?.force_terminate_requested = false;
                try self.addFailure(outgoing, correlation_id, "termination-failed", "platform force-termination request failed", true);
                return;
            };
        }
        const payload = try terminalAcknowledgedJson(self.allocator, terminal_id);
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.force_terminate_terminal, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleDispose(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        const target = try self.parseControl(TerminalTargetWire, envelope, correlation_id, outgoing) orelse return;
        defer target.deinit();
        const terminal_id = parseIdentifier(target.value.terminalId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "TerminalId is invalid", false);
            return;
        };
        const runtime = self.runtimeFor(terminal_id);
        const outcome = self.core.disposeTerminal(connection_id, terminal_id) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        if (outcome == .applied) {
            const retained_runtime = runtime orelse unreachable;
            _ = self.runtimes.orderedRemove(terminal_id);
            retained_runtime.destroy();
        }
        const payload = try terminalAcknowledgedJson(self.allocator, terminal_id);
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.dispose_terminal, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleAttach(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        const parsed = try self.parseControl(AttachWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const request = parsed.value;
        const attachment_id = parseIdentifier(request.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const terminal_id = parseIdentifier(request.terminalId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "TerminalId is invalid", false);
            return;
        };
        const capabilities: core_module.Capabilities = .{
            .input = request.capabilities.input,
            .read = request.capabilities.read,
            .resize = request.capabilities.resize,
        };
        if (protocol_minor == 0 and capabilities.read) {
            try self.addFailure(outgoing, correlation_id, "unsupported-version", "readable attachments require protocol 1.1", false);
            return;
        }
        const attach_outcome = self.core.attach(
            connection_id,
            attachment_id,
            terminal_id,
            capabilities,
            request.requestResizeLease,
        ) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        if (protocol_minor >= 2 and capabilities.read) {
            const runtime = self.runtimeFor(terminal_id) orelse {
                if (attach_outcome == .applied) _ = self.core.detach(connection_id, attachment_id) catch {};
                try self.addFailure(outgoing, correlation_id, "missing-resource", "terminal runtime is missing", false);
                return;
            };
            runtime.attachHistory(attachment_id) catch {
                if (attach_outcome == .applied) _ = self.core.detach(connection_id, attachment_id) catch {};
                try self.addFailure(outgoing, correlation_id, "resource-exhausted", "history attachment state could not be allocated", true);
                return;
            };
        }
        const attachment = self.core.findAttachment(attachment_id).?;
        var attachment_buffer: [32]u8 = undefined;
        var terminal_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":{},\"read\":{},\"resize\":{}}},\"resizeLease\":{},\"terminalId\":\"{s}\"}}",
            .{
                identifierText(attachment_id, &attachment_buffer),
                attachment.capabilities.input,
                attachment.capabilities.read,
                attachment.capabilities.resize,
                attachment.resize_lease,
                identifierText(terminal_id, &terminal_buffer),
            },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.attachment_capabilities, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleDetach(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        const parsed = try self.parseControl(DetachWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const attachment = self.core.resolveAttachment(connection_id, attachment_id, .none) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const terminal_id = attachment.terminal_id;
        if (self.runtimeFor(terminal_id)) |runtime| runtime.detachHistory(attachment_id);
        _ = self.core.detach(connection_id, attachment_id) catch unreachable;
        self.releaseProjectionIfIdle(terminal_id);
        var attachment_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"attachmentId\":\"{s}\"}}",
            .{identifierText(attachment_id, &attachment_buffer)},
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.detach_terminal, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleInput(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        negotiated_bytes: usize,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.raw_bytes)) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "WriteInput requires raw bytes", false);
            return;
        }
        const input = protocol.validateWriteInput(envelope.payload) catch |err| {
            try self.addFailure(outgoing, correlation_id, protocolErrorCode(err), "WriteInput payload is invalid", false);
            return;
        };
        if (input.bytes.len > negotiated_bytes) {
            try self.addFailure(outgoing, correlation_id, "limit-exceeded", "WriteInput exceeds negotiated inputBytes", false);
            return;
        }
        const attachment = self.core.resolveAttachment(connection_id, input.attachment_id, .input) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const terminal_id = attachment.terminal_id;
        self.synchronizeWrittenInput(terminal_id);
        _ = self.core.acceptInput(connection_id, input.attachment_id, input.bytes.len) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const runtime = self.runtimeFor(terminal_id) orelse unreachable;
        runtime.enqueueInput(input.bytes) catch |err| {
            _ = self.core.inputWritten(terminal_id, input.bytes.len) catch {};
            try self.addFailure(
                outgoing,
                correlation_id,
                switch (err) {
                    error.InputBackpressure => "input-backpressure",
                    error.TerminalExited => "invalid-phase",
                    else => "resource-exhausted",
                },
                "input could not enter the bounded write queue",
                true,
            );
            return;
        };
        var attachment_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"acceptedBytes\":{d},\"attachmentId\":\"{s}\"}}",
            .{ input.bytes.len, identifierText(input.attachment_id, &attachment_buffer) },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.input_accepted, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleSemanticInput(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        negotiated_bytes: usize,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (protocol_minor < 1) {
            try self.addFailure(outgoing, correlation_id, "unsupported-version", "semantic input requires protocol 1.1", false);
            return;
        }
        if (envelope.payload_encoding != @intFromEnum(protocol.PayloadEncoding.raw_bytes)) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "semantic input requires packed raw bytes", false);
            return;
        }
        if (envelope.payload.len < 16 or envelope.payload.len - 16 > negotiated_bytes) {
            try self.addFailure(outgoing, correlation_id, "limit-exceeded", "semantic input exceeds negotiated inputBytes", false);
            return;
        }
        const input: SemanticInput = switch (envelope.kind) {
            MessageKind.key_input => .{ .key = protocol.validateKeyInput(envelope.payload) catch |err| {
                try self.addFailure(outgoing, correlation_id, protocolErrorCode(err), "KeyInput payload is invalid", false);
                return;
            } },
            MessageKind.text_input => .{ .text = protocol.validateCommittedTextInput(envelope.payload) catch |err| {
                try self.addFailure(outgoing, correlation_id, protocolErrorCode(err), "TextInput payload is invalid", false);
                return;
            } },
            MessageKind.mouse_input => .{ .mouse = protocol.validateMouseInput(envelope.payload) catch |err| {
                try self.addFailure(outgoing, correlation_id, protocolErrorCode(err), "MouseInput payload is invalid", false);
                return;
            } },
            MessageKind.focus_input => .{ .focus = protocol.validateFocusInput(envelope.payload) catch |err| {
                try self.addFailure(outgoing, correlation_id, protocolErrorCode(err), "FocusInput payload is invalid", false);
                return;
            } },
            MessageKind.paste_input => .{ .paste = protocol.validatePasteInput(envelope.payload) catch |err| {
                try self.addFailure(outgoing, correlation_id, protocolErrorCode(err), "PasteInput payload is invalid", false);
                return;
            } },
            else => unreachable,
        };
        const attachment_id = semanticAttachmentId(input);
        const reserved_bytes = semanticReservationBytes(input) catch {
            try self.addFailure(outgoing, correlation_id, "limit-exceeded", "semantic input reservation exceeds its bound", false);
            return;
        };
        const attachment = self.core.resolveAttachment(connection_id, attachment_id, .input) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const terminal_id = attachment.terminal_id;
        self.synchronizeWrittenInput(terminal_id);
        _ = self.core.acceptInput(connection_id, attachment_id, reserved_bytes) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const runtime = self.runtimeFor(terminal_id) orelse {
            _ = self.core.inputWritten(terminal_id, reserved_bytes) catch {};
            try self.addFailure(outgoing, correlation_id, "missing-resource", "terminal runtime is missing", false);
            return;
        };
        const accepted_bytes = enqueueSemanticInput(runtime, input) catch |err| {
            _ = self.core.inputWritten(terminal_id, reserved_bytes) catch {};
            try self.addFailure(
                outgoing,
                correlation_id,
                switch (err) {
                    error.InputBackpressure => "input-backpressure",
                    error.InvalidKey => "invalid-field",
                    error.TerminalExited => "invalid-phase",
                    else => "resource-exhausted",
                },
                "semantic input could not enter the bounded write queue",
                err == error.InputBackpressure,
            );
            return;
        };
        if (accepted_bytes < reserved_bytes) {
            _ = self.core.inputWritten(
                terminal_id,
                reserved_bytes - accepted_bytes,
            ) catch {};
        }
        var attachment_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"acceptedBytes\":{d},\"attachmentId\":\"{s}\"}}",
            .{ accepted_bytes, identifierText(attachment_id, &attachment_buffer) },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.input_accepted, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleFrameApplied(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_1(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(FrameAppliedWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const generation = parseCanonicalU64(parsed.value.generation) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "generation is not canonical uint64 text", false);
            return;
        };
        const revision = parseCanonicalU64(parsed.value.revision) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "revision is not canonical uint64 text", false);
            return;
        };
        const sequence = parseCanonicalU64(parsed.value.sequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "sequence is not canonical uint64 text", false);
            return;
        };
        _ = self.core.frameApplied(
            connection_id,
            attachment_id,
            sequence,
            revision,
            generation,
        ) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        try outgoing.add(MessageKind.frame_applied, .control_json, envelope.payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleNeedSnapshot(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_1(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(NeedSnapshotWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        if (!validSnapshotReason(parsed.value.reason)) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "NeedSnapshot reason is invalid", false);
            return;
        }
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const sequence = parseCanonicalU64(parsed.value.sequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "sequence is not canonical uint64 text", false);
            return;
        };
        _ = self.core.needSnapshot(connection_id, attachment_id, sequence) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        try outgoing.add(MessageKind.need_snapshot, .control_json, envelope.payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleProjectionDemand(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_1(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SetProjectionDemandWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const demand: core_module.ProjectionDemand = if (std.mem.eql(u8, parsed.value.demand, "active"))
            .active
        else if (std.mem.eql(u8, parsed.value.demand, "paused"))
            .paused
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "projection demand is invalid", false);
            return;
        };
        const attachment = self.core.resolveAttachment(connection_id, attachment_id, .read) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const terminal_id = attachment.terminal_id;
        _ = self.core.setProjectionDemand(connection_id, attachment_id, demand) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        self.releaseProjectionIfIdle(terminal_id);
        var attachment_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"attachmentId\":\"{s}\",\"demand\":\"{s}\"}}",
            .{ identifierText(attachment_id, &attachment_buffer), @tagName(demand) },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.projection_demand_accepted, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleReadRows(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        negotiated_bytes: usize,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(ReadRowsWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const request = parsed.value;
        const attachment_id = parseIdentifier(request.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        if (request.maximumBytes > negotiated_bytes) {
            try self.addFailure(outgoing, correlation_id, "limit-exceeded", "ReadRows exceeds negotiated rowWindowBytes", false);
            return;
        }
        const revision = parseCanonicalU64(request.revision) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "revision is not canonical uint64 text", false);
            return;
        };
        const generation = parseCanonicalU64(request.generation) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "generation is not canonical uint64 text", false);
            return;
        };
        const sequence = parseCanonicalU64(request.requestSequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "requestSequence is not canonical uint64 text", false);
            return;
        };
        const detail: terminal_history.Detail = if (std.mem.eql(u8, request.detail, "render"))
            .render
        else if (std.mem.eql(u8, request.detail, "text"))
            .text
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "ReadRows detail is invalid", false);
            return;
        };
        const purpose: terminal_history.Purpose = if (std.mem.eql(u8, request.purpose, "viewport"))
            .viewport
        else if (std.mem.eql(u8, request.purpose, "search"))
            .search
        else if (std.mem.eql(u8, request.purpose, "buffer"))
            .buffer
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "ReadRows purpose is invalid", false);
            return;
        };
        const origin = parseRowOrigin(request.origin) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "ReadRows origin is invalid", false);
            return;
        };
        const rows = runtime.readHistoryRows(.{
            .attachment_id = attachment_id,
            .detail = detail,
            .generation = generation,
            .host_epoch = self.core.epoch,
            .maximum_bytes = request.maximumBytes,
            .origin = origin,
            .purpose = purpose,
            .request_sequence = sequence,
            .revision = revision,
            .row_count = request.rowCount,
        }) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "ReadRows failed");
            return;
        };
        defer self.allocator.free(rows.bytes);
        try outgoing.add(
            MessageKind.rows_snapshot,
            .row_window,
            rows.bytes,
            correlation_id,
            self.core.epoch,
            true,
            false,
        );
    }

    fn handleRowsApplied(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(RowsAppliedWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const revision = parseCanonicalU64(parsed.value.revision) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "revision is not canonical uint64 text", false);
            return;
        };
        const generation = parseCanonicalU64(parsed.value.generation) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "generation is not canonical uint64 text", false);
            return;
        };
        const sequence = parseCanonicalU64(parsed.value.requestSequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "requestSequence is not canonical uint64 text", false);
            return;
        };
        runtime.historyRowsApplied(attachment_id, sequence, revision, generation) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "RowsApplied failed");
            return;
        };
        try outgoing.add(MessageKind.rows_applied, .control_json, envelope.payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleHistoryDemand(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SetHistoryDemandWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const demand: terminal_runtime.HistoryDemand = if (std.mem.eql(u8, parsed.value.demand, "following-bottom"))
            .following_bottom
        else if (std.mem.eql(u8, parsed.value.demand, "viewing-history"))
            .viewing_history
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "history demand is invalid", false);
            return;
        };
        runtime.setHistoryDemand(attachment_id, demand) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "SetHistoryDemand failed");
            return;
        };
        try outgoing.add(MessageKind.history_demand_accepted, .control_json, envelope.payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleHistoryObserved(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(HistoryObservedWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const revision = parseCanonicalU64(parsed.value.revision) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "revision is not canonical uint64 text", false);
            return;
        };
        const generation = parseCanonicalU64(parsed.value.generation) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "generation is not canonical uint64 text", false);
            return;
        };
        runtime.historyObserved(attachment_id, revision, generation) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "HistoryObserved failed");
            return;
        };
        try outgoing.add(MessageKind.history_observed, .control_json, envelope.payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleSelectionGesture(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SelectionGestureWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const action: terminal_history.SelectionGestureAction = if (std.mem.eql(u8, parsed.value.action, "start"))
            .start
        else if (std.mem.eql(u8, parsed.value.action, "update"))
            .update
        else if (std.mem.eql(u8, parsed.value.action, "end"))
            .end
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "selection gesture action is invalid", false);
            return;
        };
        const point = parseSelectionPoint(parsed.value.point) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "selection point is invalid", false);
            return;
        };
        const revision = parseCanonicalU64(parsed.value.revision) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "revision is not canonical uint64 text", false);
            return;
        };
        const generation = parseCanonicalU64(parsed.value.generation) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "generation is not canonical uint64 text", false);
            return;
        };
        const snapshot = runtime.applySelectionGesture(.{
            .action = action,
            .attachment_id = attachment_id,
            .click_count = parsed.value.clickCount,
            .generation = generation,
            .point = point,
            .rectangle = parsed.value.rectangle,
            .revision = revision,
        }) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "SelectionGesture failed");
            return;
        };
        try self.addSelectionChanged(outgoing, correlation_id, attachment_id, snapshot);
    }

    fn handleSelectionCommand(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SelectionCommandWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const command: terminal_history.SelectionCommand = if (std.mem.eql(u8, parsed.value.command, "clear") and parsed.value.point == null)
            .clear
        else if (std.mem.eql(u8, parsed.value.command, "select-all") and parsed.value.point == null)
            .select_all
        else if (std.mem.eql(u8, parsed.value.command, "select-output"))
            .{ .select_output = parseSelectionPoint(parsed.value.point orelse {
                try self.addFailure(outgoing, correlation_id, "invalid-field", "select-output requires a point", false);
                return;
            }) catch {
                try self.addFailure(outgoing, correlation_id, "invalid-field", "selection point is invalid", false);
                return;
            } }
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "selection command is invalid", false);
            return;
        };
        const revision = parseCanonicalU64(parsed.value.revision) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "revision is not canonical uint64 text", false);
            return;
        };
        const generation = parseCanonicalU64(parsed.value.generation) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "generation is not canonical uint64 text", false);
            return;
        };
        const snapshot = runtime.applySelectionCommand(.{
            .attachment_id = attachment_id,
            .command = command,
            .generation = generation,
            .revision = revision,
        }) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "SelectionCommand failed");
            return;
        };
        try self.addSelectionChanged(outgoing, correlation_id, attachment_id, snapshot);
    }

    fn handleSelectionExport(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SelectionExportWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const export_sequence = parseCanonicalU64(parsed.value.exportSequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "exportSequence is not canonical uint64 text", false);
            return;
        };
        const format: terminal_runtime.SelectionExportFormat = if (std.mem.eql(u8, parsed.value.format, "plain"))
            .plain
        else if (std.mem.eql(u8, parsed.value.format, "html"))
            .html
        else if (std.mem.eql(u8, parsed.value.format, "vt"))
            .vt
        else {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "selection export format is invalid", false);
            return;
        };
        const result = runtime.startSelectionExport(attachment_id, export_sequence, format) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "SelectionExport failed");
            return;
        };
        const started = result.started;
        if (self.trace_interactions) {
            std.debug.print(
                "{{\"byteLength\":{d},\"lockHoldNanoseconds\":{d},\"type\":\"orchbee.terminal-host.selection-export\"}}\n",
                .{ started.byte_length, result.lock_hold_nanoseconds },
            );
        }
        var attachment_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"format\":\"{s}\"}}",
            .{
                identifierText(attachment_id, &attachment_buffer),
                started.byte_length,
                started.export_sequence,
                @tagName(started.format),
            },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.selection_export_started, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn handleSelectionExportChunkApplied(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SelectionExportChunkAppliedWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const export_sequence = parseCanonicalU64(parsed.value.exportSequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "exportSequence is not canonical uint64 text", false);
            return;
        };
        const finished = runtime.selectionExportChunkApplied(
            attachment_id,
            export_sequence,
            parsed.value.chunkSequence,
        ) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "SelectionExportChunkApplied failed");
            return;
        };
        try outgoing.add(MessageKind.selection_export_chunk_applied, .control_json, envelope.payload, correlation_id, self.core.epoch, true, false);
        if (finished) |value| try self.addSelectionExportFinished(outgoing, value, "completed", false, null);
    }

    fn handleSelectionExportCancel(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        if (!try self.requireProtocolV1_2(protocol_minor, correlation_id, outgoing)) return;
        const parsed = try self.parseControl(SelectionExportCancelWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        if (!validExportCancelReason(parsed.value.reason)) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "selection export cancel reason is invalid", false);
            return;
        }
        const attachment_id = parseIdentifier(parsed.value.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const runtime = try self.requireHistoryRuntime(connection_id, attachment_id, correlation_id, outgoing) orelse return;
        const export_sequence = parseCanonicalU64(parsed.value.exportSequence) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "exportSequence is not canonical uint64 text", false);
            return;
        };
        const finished = runtime.cancelSelectionExport(attachment_id, export_sequence) catch |err| {
            try self.addInteractionFailure(outgoing, correlation_id, err, "SelectionExportCancel failed");
            return;
        };
        try self.addSelectionExportFinished(outgoing, finished, "cancelled", true, correlation_id);
    }

    fn handleResize(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        protocol_minor: u16,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !void {
        const parsed = try self.parseControl(ResizeWire, envelope, correlation_id, outgoing) orelse return;
        defer parsed.deinit();
        const request = parsed.value;
        if (!validResize(request, protocol_minor)) {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "terminal dimensions are invalid", false);
            return;
        }
        const attachment_id = parseIdentifier(request.attachmentId) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-identifier", "AttachmentId is invalid", false);
            return;
        };
        const attachment = self.core.resolveAttachment(connection_id, attachment_id, .resize) catch {
            try self.addFailure(outgoing, correlation_id, "resize-lease-denied", "attachment does not own the resize lease", false);
            return;
        };
        if (!attachment.resize_lease) {
            try self.addFailure(outgoing, correlation_id, "resize-lease-denied", "attachment does not own the resize lease", false);
            return;
        }
        const terminal_id = attachment.terminal_id;
        const terminal = self.core.findTerminal(terminal_id) orelse unreachable;
        const runtime = self.runtimeFor(terminal_id) orelse {
            try self.addFailure(outgoing, correlation_id, "missing-resource", "terminal runtime is missing", false);
            return;
        };
        const old_cell_pixel_height = terminal.cell_pixel_height;
        const old_cell_pixel_width = terminal.cell_pixel_width;
        _ = self.core.resize(connection_id, attachment_id, request.columns, request.rows) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        const cell_pixel_height = request.cellPixelHeight orelse old_cell_pixel_height;
        const cell_pixel_width = request.cellPixelWidth orelse old_cell_pixel_width;
        const revision = runtime.resizeGeometry(
            request.columns,
            request.rows,
            cell_pixel_width,
            cell_pixel_height,
        ) catch {
            try self.addFailure(outgoing, correlation_id, "resize-failed", "PTY resize failed", true);
            return;
        };
        self.core.synchronizeGeometry(
            terminal_id,
            request.columns,
            request.rows,
            cell_pixel_width,
            cell_pixel_height,
            revision,
        ) catch |err| {
            try self.addCoreFailure(outgoing, correlation_id, err);
            return;
        };
        var attachment_buffer: [32]u8 = undefined;
        const payload = if (protocol_minor == 0)
            try std.fmt.allocPrint(
                self.allocator,
                "{{\"attachmentId\":\"{s}\",\"columns\":{d},\"revision\":\"{d}\",\"rows\":{d}}}",
                .{
                    identifierText(attachment_id, &attachment_buffer),
                    request.columns,
                    revision,
                    request.rows,
                },
            )
        else
            try std.fmt.allocPrint(
                self.allocator,
                "{{\"attachmentId\":\"{s}\",\"cellPixelHeight\":{d},\"cellPixelWidth\":{d},\"columns\":{d},\"revision\":\"{d}\",\"rows\":{d}}}",
                .{
                    identifierText(attachment_id, &attachment_buffer),
                    cell_pixel_height,
                    cell_pixel_width,
                    request.columns,
                    revision,
                    request.rows,
                },
            );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.resize_accepted, .control_json, payload, correlation_id, self.core.epoch, true, false);
    }

    fn synchronizeRuntimes(
        self: *HostService,
        outgoing: *Outgoing,
        emit_events: bool,
    ) !void {
        for (self.runtimes.keys(), self.runtimes.values()) |terminal_id, *entry| {
            self.synchronizeWrittenInput(terminal_id);
            const completion = entry.runtime.completion();
            if (self.core.findTerminal(terminal_id)) |terminal| {
                _ = self.core.synchronizeRevision(terminal_id, completion.final_revision) catch {};
                const was_exited = terminal.phase == .exited;
                if (completion.pty_drained and !terminal.pty_drained) {
                    _ = self.core.ptyDrained(terminal_id) catch {};
                }
                if (completion.child_exited and !terminal.child_exited) {
                    _ = self.core.childExited(
                        terminal_id,
                        coreExitResult(completion.exit_result.?),
                    ) catch {};
                }
                if (emit_events and !was_exited and self.core.findTerminal(terminal_id).?.phase == .exited) {
                    const payload = try self.terminalExitedJson(self.core.findTerminal(terminal_id).?.*);
                    defer self.allocator.free(payload);
                    try outgoing.add(MessageKind.terminal_exited, .control_json, payload, null, self.core.epoch, false, true);
                }
            }
        }
    }

    fn scheduleInteraction(
        self: *HostService,
        connection: *const ConnectionState,
        outgoing: *Outgoing,
    ) !void {
        const connection_id = connection.connection_id orelse return;
        if (connection.protocol_minor < 2) return;
        const count = self.core.attachments.count();
        if (count == 0) return;
        const start = self.interaction_schedule_index % count;
        var scanned: usize = 0;
        var scheduled: usize = 0;
        while (scanned < @min(count, max_schedule_scan_per_poll) and
            scheduled < max_scheduled_attachments_per_poll)
        {
            const index = (start + scanned) % count;
            scanned += 1;
            const attachment = self.core.attachments.values()[index];
            if (attachment.connection_id != connection_id or !attachment.capabilities.read) {
                continue;
            }
            const runtime = self.runtimeFor(attachment.terminal_id) orelse continue;
            scheduled += 1;
            if (try runtime.takeHistoryEvent(attachment.attachment_id)) |event| {
                var attachment_buffer: [32]u8 = undefined;
                const payload = try std.fmt.allocPrint(
                    self.allocator,
                    "{{\"anchorStatus\":\"{s}\",\"attachmentId\":\"{s}\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"totalRows\":\"{d}\"}}",
                    .{
                        switch (event.anchor_status) {
                            .evicted => "evicted",
                            .none => "none",
                            .valid => "valid",
                        },
                        identifierText(event.attachment_id, &attachment_buffer),
                        event.generation,
                        event.revision,
                        event.total_rows,
                    },
                );
                defer self.allocator.free(payload);
                try outgoing.add(
                    MessageKind.history_changed,
                    .control_json,
                    payload,
                    null,
                    self.core.epoch,
                    false,
                    true,
                );
            }
            if (try runtime.takeSelectionEvent(attachment.attachment_id)) |selection| {
                try self.addSelectionChanged(
                    outgoing,
                    null,
                    attachment.attachment_id,
                    selection,
                );
            }
            if (try runtime.nextSelectionExportChunk(attachment.attachment_id)) |chunk| {
                defer self.allocator.free(chunk);
                try outgoing.add(
                    MessageKind.selection_export_chunk,
                    .raw_bytes,
                    chunk,
                    null,
                    self.core.epoch,
                    false,
                    true,
                );
            }
        }
        self.interaction_schedule_index = (start + scanned) % count;
    }

    fn scheduleProjection(
        self: *HostService,
        connection: *const ConnectionState,
        outgoing: *Outgoing,
    ) !void {
        const connection_id = connection.connection_id orelse return;
        if (connection.protocol_minor < 1) return;
        const count = self.core.attachments.count();
        if (count == 0) return;
        const now = std.time.nanoTimestamp();
        const start = self.projection_schedule_index % count;
        var scanned: usize = 0;
        var scheduled: usize = 0;
        var used_terminal_ids: [max_scheduled_attachments_per_poll]core_module.Identifier = undefined;
        var used_terminal_count: usize = 0;
        while (scanned < @min(count, max_schedule_scan_per_poll) and
            scheduled < max_scheduled_attachments_per_poll)
        {
            const index = (start + scanned) % count;
            scanned += 1;
            const attachment = &self.core.attachments.values()[index];
            if (attachment.connection_id != connection_id or
                !attachment.capabilities.read or attachment.demand != .active or
                attachment.in_flight != null or attachment.blocked != null or
                (!attachment.pending_dirty and !attachment.force_snapshot))
            {
                continue;
            }
            const entry = self.runtimes.getPtr(attachment.terminal_id) orelse continue;
            if (now < entry.next_projection_at_ns) continue;
            const runtime = entry.runtime;
            const terminal = self.core.findTerminal(attachment.terminal_id) orelse continue;
            scheduled += 1;
            const geometry = runtime.geometry();
            _ = self.core.synchronizeRevision(terminal.terminal_id, geometry.revision) catch {};
            const force_snapshot = attachment.force_snapshot or
                attachment.acknowledged_revision == null or
                attachment.acknowledged_generation == null or
                attachment.acknowledged_generation.? != geometry.generation;
            if (!force_snapshot and attachment.acknowledged_revision.? >= geometry.revision) {
                self.core.projectionCaughtUp(attachment.attachment_id) catch {};
                continue;
            }
            var already_used = false;
            for (used_terminal_ids[0..used_terminal_count]) |used_terminal_id| {
                already_used = already_used or identifierEqual(used_terminal_id, attachment.terminal_id);
            }
            if (!already_used) {
                used_terminal_ids[used_terminal_count] = attachment.terminal_id;
                used_terminal_count += 1;
            }
            const encoded = runtime.encodeProjectionFrame(.{
                .attachment_id = attachment.attachment_id,
                .base_generation = attachment.acknowledged_generation,
                .base_revision = attachment.acknowledged_revision,
                .force_snapshot = force_snapshot,
                .host_epoch = self.core.epoch,
                .maximum_bytes = connection.frame_bytes,
                .sequence = attachment.next_sequence,
            }) catch |err| {
                const reason: core_module.ProjectionBlock = switch (err) {
                    error.ProjectionTooLarge => .projection_too_large,
                    else => .capture_allocation_failed,
                };
                self.core.blockProjection(attachment.attachment_id, reason) catch {};
                const blocked_geometry = runtime.geometry();
                _ = self.core.synchronizeRevision(attachment.terminal_id, blocked_geometry.revision) catch {};
                try self.addProjectionBlocked(
                    outgoing,
                    attachment.attachment_id,
                    reason,
                    blocked_geometry.revision,
                );
                continue;
            };
            defer self.allocator.free(encoded.bytes);
            _ = self.core.synchronizeRevision(attachment.terminal_id, encoded.revision) catch {};
            const message_kind = switch (encoded.kind) {
                .snapshot => MessageKind.snapshot_frame,
                .delta => MessageKind.delta_frame,
            };
            try outgoing.add(
                message_kind,
                .terminal_frame,
                encoded.bytes,
                null,
                self.core.epoch,
                false,
                true,
            );
            const flight = self.core.commitFrame(
                attachment.attachment_id,
                encoded.base_revision,
                encoded.revision,
                encoded.generation,
                switch (encoded.kind) {
                    .snapshot => .snapshot,
                    .delta => .delta,
                },
            ) catch |err| {
                const record = outgoing.records.pop().?;
                self.allocator.free(record);
                return err;
            };
            std.debug.assert(flight.sequence == encoded.sequence);
        }
        self.projection_schedule_index = (start + scanned) % count;
        for (used_terminal_ids[0..used_terminal_count]) |terminal_id| {
            if (self.runtimes.getPtr(terminal_id)) |entry| {
                entry.next_projection_at_ns = now + projection_interval_ns;
            }
        }
    }

    fn releaseProjectionIfIdle(self: *HostService, terminal_id: core_module.Identifier) void {
        if (self.core.hasActiveProjectionDemand(terminal_id)) return;
        if (self.runtimeFor(terminal_id)) |runtime| runtime.releaseProjectionResources();
    }

    fn addProjectionBlocked(
        self: *HostService,
        outgoing: *Outgoing,
        attachment_id: core_module.Identifier,
        reason: core_module.ProjectionBlock,
        revision: u64,
    ) !void {
        var attachment_buffer: [32]u8 = undefined;
        const reason_text = switch (reason) {
            .capture_allocation_failed => "capture-allocation-failed",
            .projection_too_large => "projection-too-large",
        };
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"attachmentId\":\"{s}\",\"reason\":\"{s}\",\"revision\":\"{d}\"}}",
            .{ identifierText(attachment_id, &attachment_buffer), reason_text, revision },
        );
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.projection_blocked, .control_json, payload, null, self.core.epoch, false, true);
    }

    fn synchronizeWrittenInput(self: *HostService, terminal_id: core_module.Identifier) void {
        const runtime = self.runtimeFor(terminal_id) orelse return;
        const written = runtime.takeWrittenBytes();
        if (written == 0 or self.core.findTerminal(terminal_id) == null) return;
        _ = self.core.inputWritten(terminal_id, written) catch {};
    }

    fn runtimeFor(
        self: *HostService,
        terminal_id: core_module.Identifier,
    ) ?*terminal_runtime.TerminalRuntime {
        const entry = self.runtimes.getPtr(terminal_id) orelse return null;
        return entry.runtime;
    }

    pub fn findRuntime(
        self: *HostService,
        terminal_id: core_module.Identifier,
    ) ?*terminal_runtime.TerminalRuntime {
        return self.runtimeFor(terminal_id);
    }

    fn requireControl(
        self: *HostService,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !bool {
        if (envelope.payload_encoding == @intFromEnum(protocol.PayloadEncoding.control_json)) return true;
        try self.addFailure(outgoing, correlation_id, "invalid-field", "message requires control JSON", false);
        return false;
    }

    fn requireProtocolV1_1(
        self: *HostService,
        protocol_minor: u16,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !bool {
        if (protocol_minor >= 1) return true;
        try self.addFailure(outgoing, correlation_id, "unsupported-version", "operation requires protocol 1.1", false);
        return false;
    }

    fn requireProtocolV1_2(
        self: *HostService,
        protocol_minor: u16,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !bool {
        if (protocol_minor >= 2) return true;
        try self.addFailure(outgoing, correlation_id, "unsupported-version", "operation requires protocol 1.2", false);
        return false;
    }

    fn requireProtocolV1_4(
        self: *HostService,
        protocol_minor: u16,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !bool {
        if (protocol_minor >= 4) return true;
        try self.addFailure(outgoing, correlation_id, "unsupported-version", "operation requires protocol 1.4", false);
        return false;
    }

    fn requireHistoryRuntime(
        self: *HostService,
        connection_id: core_module.ConnectionId,
        attachment_id: core_module.Identifier,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !?*terminal_runtime.TerminalRuntime {
        const attachment = self.core.resolveAttachment(connection_id, attachment_id, .read) catch {
            try self.addFailure(outgoing, correlation_id, "missing-resource", "attachment has no readable history capability", false);
            return null;
        };
        return self.runtimeFor(attachment.terminal_id) orelse {
            try self.addFailure(outgoing, correlation_id, "missing-resource", "terminal runtime is missing", false);
            return null;
        };
    }

    fn addSelectionChanged(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: ?core_module.Identifier,
        attachment_id: core_module.Identifier,
        snapshot: terminal_runtime.SelectionSnapshot,
    ) !void {
        var attachment_buffer: [32]u8 = undefined;
        const attachment_text = identifierText(attachment_id, &attachment_buffer);
        const payload = switch (snapshot) {
            .none => try std.fmt.allocPrint(
                self.allocator,
                "{{\"attachmentId\":\"{s}\",\"state\":\"none\"}}",
                .{attachment_text},
            ),
            .evicted => try std.fmt.allocPrint(
                self.allocator,
                "{{\"attachmentId\":\"{s}\",\"state\":\"evicted\"}}",
                .{attachment_text},
            ),
            .active => |active| try std.fmt.allocPrint(
                self.allocator,
                "{{\"attachmentId\":\"{s}\",\"end\":{{\"column\":{d},\"row\":\"{d}\"}},\"preview\":null,\"rectangle\":{},\"start\":{{\"column\":{d},\"row\":\"{d}\"}},\"state\":\"active\"}}",
                .{
                    attachment_text,
                    active.end.column,
                    active.end.row,
                    active.rectangle,
                    active.start.column,
                    active.start.row,
                },
            ),
        };
        defer self.allocator.free(payload);
        const response = correlation_id != null;
        try outgoing.add(
            MessageKind.selection_changed,
            .control_json,
            payload,
            correlation_id,
            self.core.epoch,
            response,
            !response,
        );
    }

    fn addSelectionExportFinished(
        self: *HostService,
        outgoing: *Outgoing,
        finished: terminal_runtime.SelectionExportFinished,
        status: []const u8,
        response: bool,
        correlation_id: ?core_module.Identifier,
    ) !void {
        var attachment_buffer: [32]u8 = undefined;
        const payload = try std.fmt.allocPrint(
            self.allocator,
            "{{\"attachmentId\":\"{s}\",\"byteLength\":{d},\"exportSequence\":\"{d}\",\"status\":\"{s}\"}}",
            .{
                identifierText(finished.attachment_id, &attachment_buffer),
                finished.byte_length,
                finished.export_sequence,
                status,
            },
        );
        defer self.allocator.free(payload);
        try outgoing.add(
            MessageKind.selection_export_finished,
            .control_json,
            payload,
            correlation_id,
            self.core.epoch,
            response,
            !response,
        );
    }

    fn addInteractionFailure(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: core_module.Identifier,
        err: anyerror,
        message: []const u8,
    ) !void {
        const mapped: struct { code: []const u8, retryable: bool } = switch (err) {
            error.AnchorEvicted => .{ .code = "history-evicted", .retryable = true },
            error.ExportConflict => .{ .code = "export-in-progress", .retryable = true },
            error.ExportTooLarge, error.ProjectionTooLarge => .{ .code = "limit-exceeded", .retryable = false },
            error.InvalidAcknowledgement => .{ .code = "invalid-acknowledgement", .retryable = false },
            error.InvalidPoint, error.InvalidRequest => .{ .code = "invalid-field", .retryable = false },
            error.MissingAttachment, error.MissingSelection => .{ .code = "missing-resource", .retryable = false },
            error.StaleGeneration => .{ .code = "stale-generation", .retryable = true },
            error.StaleRevision => .{ .code = "stale-revision", .retryable = true },
            error.OutOfMemory => .{ .code = "resource-exhausted", .retryable = true },
            else => .{ .code = "interaction-failed", .retryable = false },
        };
        try self.addFailure(outgoing, correlation_id, mapped.code, message, mapped.retryable);
    }

    fn requireEmptyControl(
        self: *HostService,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !bool {
        if (!try self.requireControl(envelope, correlation_id, outgoing)) return false;
        const parsed = std.json.parseFromSlice(struct {}, self.allocator, envelope.payload, .{}) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "message requires an empty control object", false);
            return false;
        };
        parsed.deinit();
        return true;
    }

    fn parseControl(
        self: *HostService,
        comptime Wire: type,
        envelope: protocol.EnvelopeSummary,
        correlation_id: core_module.Identifier,
        outgoing: *Outgoing,
    ) !?std.json.Parsed(Wire) {
        if (!try self.requireControl(envelope, correlation_id, outgoing)) return null;
        return std.json.parseFromSlice(Wire, self.allocator, envelope.payload, .{}) catch {
            try self.addFailure(outgoing, correlation_id, "invalid-field", "control payload is invalid", false);
            return null;
        };
    }

    fn addCoreFailure(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: core_module.Identifier,
        err: anyerror,
    ) !void {
        const mapped = coreError(err);
        try self.addFailure(outgoing, correlation_id, mapped.code, mapped.message, mapped.retryable);
    }

    fn addFailure(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: ?core_module.Identifier,
        code: []const u8,
        message: []const u8,
        retryable: bool,
    ) !void {
        try self.addFailureWithEpoch(
            outgoing,
            correlation_id,
            code,
            message,
            retryable,
            self.core.epoch,
        );
    }

    fn addHandshakeFailure(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: ?core_module.Identifier,
        code: []const u8,
        message: []const u8,
        retryable: bool,
    ) !void {
        try self.addFailureWithEpoch(
            outgoing,
            correlation_id,
            code,
            message,
            retryable,
            null,
        );
    }

    fn addFailureWithEpoch(
        self: *HostService,
        outgoing: *Outgoing,
        correlation_id: ?core_module.Identifier,
        code: []const u8,
        message: []const u8,
        retryable: bool,
        host_epoch: ?core_module.Identifier,
    ) !void {
        var writer: std.Io.Writer.Allocating = .init(self.allocator);
        defer writer.deinit();
        try writer.writer.writeAll("{\"code\":");
        try std.json.Stringify.value(code, .{}, &writer.writer);
        try writer.writer.writeAll(",\"message\":");
        try std.json.Stringify.value(message, .{}, &writer.writer);
        try writer.writer.print(",\"retryable\":{}}}", .{retryable});
        const payload = try writer.toOwnedSlice();
        defer self.allocator.free(payload);
        try outgoing.add(MessageKind.typed_failure, .control_json, payload, correlation_id, host_epoch, true, false);
    }

    fn identityJson(
        self: *HostService,
        creation_id: core_module.Identifier,
        terminal_id: core_module.Identifier,
    ) ![]u8 {
        var creation_buffer: [32]u8 = undefined;
        var epoch_buffer: [32]u8 = undefined;
        var terminal_buffer: [32]u8 = undefined;
        return std.fmt.allocPrint(
            self.allocator,
            "{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}}",
            .{
                identifierText(creation_id, &creation_buffer),
                identifierText(self.core.epoch, &epoch_buffer),
                identifierText(terminal_id, &terminal_buffer),
            },
        );
    }

    fn terminalSummaryJson(self: *HostService, terminal: core_module.Terminal) ![]u8 {
        var creation_buffer: [32]u8 = undefined;
        var epoch_buffer: [32]u8 = undefined;
        var terminal_buffer: [32]u8 = undefined;
        const exit_json = try exitResultJson(self.allocator, terminal.exit_result);
        defer self.allocator.free(exit_json);
        return std.fmt.allocPrint(
            self.allocator,
            "{{\"columns\":{d},\"exit\":{s},\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}},\"phase\":\"{s}\",\"revision\":\"{d}\",\"rows\":{d},\"terminateRequested\":{}}}",
            .{
                terminal.columns,
                exit_json,
                identifierText(terminal.creation_id, &creation_buffer),
                identifierText(self.core.epoch, &epoch_buffer),
                identifierText(terminal.terminal_id, &terminal_buffer),
                @tagName(terminal.phase),
                terminal.revision,
                terminal.rows,
                terminal.terminate_requested,
            },
        );
    }

    fn terminalListJsonLegacy(self: *HostService) ![]u8 {
        if (self.core.terminalCount() == 0) return self.allocator.dupe(u8, "{\"terminals\":[]}");
        std.debug.assert(self.core.terminalCount() == 1);
        const summary = try self.terminalSummaryJson(self.core.terminals.values()[0]);
        defer self.allocator.free(summary);
        return std.fmt.allocPrint(self.allocator, "{{\"terminals\":[{s}]}}", .{summary});
    }

    fn terminalListJsonV1_3(
        self: *HostService,
        offset: usize,
        maximum_terminals: usize,
    ) ![]u8 {
        const page_length = @min(maximum_terminals, self.core.terminalCount() - offset);
        const end = offset + page_length;
        var writer: std.Io.Writer.Allocating = .init(self.allocator);
        defer writer.deinit();
        try writer.writer.print(
            "{{\"inventoryGeneration\":\"{d}\",\"nextCursor\":",
            .{self.core.inventory_generation},
        );
        if (end < self.core.terminalCount()) {
            try writer.writer.print(
                "{{\"inventoryGeneration\":\"{d}\",\"offset\":\"{d}\"}}",
                .{ self.core.inventory_generation, end },
            );
        } else {
            try writer.writer.writeAll("null");
        }
        try writer.writer.writeAll(",\"terminals\":[");
        for (self.core.terminals.values()[offset..end], 0..) |terminal, index| {
            if (index != 0) try writer.writer.writeByte(',');
            const summary = try self.terminalSummaryJson(terminal);
            defer self.allocator.free(summary);
            try writer.writer.writeAll(summary);
        }
        try writer.writer.writeAll("]}");
        const payload = try writer.toOwnedSlice();
        errdefer self.allocator.free(payload);
        if (payload.len > max_control_json_bytes) return error.InventoryPageTooLarge;
        return payload;
    }

    fn terminalExitedJson(self: *HostService, terminal: core_module.Terminal) ![]u8 {
        var creation_buffer: [32]u8 = undefined;
        var epoch_buffer: [32]u8 = undefined;
        var terminal_buffer: [32]u8 = undefined;
        const exit_json = try exitResultJson(self.allocator, terminal.exit_result);
        defer self.allocator.free(exit_json);
        return std.fmt.allocPrint(
            self.allocator,
            "{{\"exit\":{s},\"finalRevision\":\"{d}\",\"identity\":{{\"creationId\":\"{s}\",\"hostEpoch\":\"{s}\",\"terminalId\":\"{s}\"}}}}",
            .{
                exit_json,
                terminal.revision,
                identifierText(terminal.creation_id, &creation_buffer),
                identifierText(self.core.epoch, &epoch_buffer),
                identifierText(terminal.terminal_id, &terminal_buffer),
            },
        );
    }
};

fn validHello(hello: HelloWire) bool {
    if (hello.frameBytes == 0 or hello.frameBytes > 8 * 1024 * 1024 or
        hello.inputBytes == 0 or hello.inputBytes > 1024 * 1024 or
        hello.rowWindowBytes == 0 or hello.rowWindowBytes > 2 * 1024 * 1024 or
        hello.protocol.minimumMinor > hello.protocol.maximumMinor or
        hello.supportedOptionalFeatures.len > 64)
    {
        return false;
    }
    for (hello.supportedOptionalFeatures) |feature| {
        if (feature.len == 0 or feature.len > 64) return false;
    }
    return true;
}

fn validLaunch(launch: LaunchWire, protocol_minor: u16) bool {
    if (launch.executable.len == 0 or containsNul(launch.executable) or
        !validDimension(launch.columns) or !validDimension(launch.rows) or
        @as(u32, launch.columns) * launch.rows > 256 * 1024 or
        launch.arguments.len > 4096 or launch.environment.len > 1024)
    {
        return false;
    }
    if (protocol_minor == 0) {
        if (launch.cellPixelHeight != null or launch.cellPixelWidth != null or
            launch.terminalProfile != null) return false;
    } else {
        const cell_pixel_height = launch.cellPixelHeight orelse return false;
        const cell_pixel_width = launch.cellPixelWidth orelse return false;
        const profile = launch.terminalProfile orelse return false;
        if (!validPixelGeometry(
            launch.columns,
            launch.rows,
            cell_pixel_width,
            cell_pixel_height,
        ) or !validTerminalProfile(profile)) return false;
    }
    if (launch.currentDirectory) |directory| if (containsNul(directory)) return false;
    for (launch.arguments) |argument| if (containsNul(argument)) return false;
    for (launch.environment, 0..) |entry, index| {
        if (entry.name.len == 0 or entry.name.len > 32_768 or containsNul(entry.name) or
            std.mem.indexOfScalar(u8, entry.name, '=') != null or
            (entry.value != null and containsNul(entry.value.?)))
        {
            return false;
        }
        for (launch.environment[0..index]) |previous| {
            if (std.mem.eql(u8, previous.name, entry.name)) return false;
        }
    }
    return true;
}

fn validResize(resize: ResizeWire, protocol_minor: u16) bool {
    if (!validDimension(resize.columns) or !validDimension(resize.rows) or
        @as(u32, resize.columns) * resize.rows > 256 * 1024)
    {
        return false;
    }
    if (protocol_minor == 0) {
        return resize.cellPixelHeight == null and resize.cellPixelWidth == null;
    }
    const cell_pixel_height = resize.cellPixelHeight orelse return false;
    const cell_pixel_width = resize.cellPixelWidth orelse return false;
    return validPixelGeometry(
        resize.columns,
        resize.rows,
        cell_pixel_width,
        cell_pixel_height,
    );
}

fn validPixelGeometry(
    columns: u16,
    rows: u16,
    cell_pixel_width: u16,
    cell_pixel_height: u16,
) bool {
    return cell_pixel_width >= 1 and cell_pixel_width <= 4096 and
        cell_pixel_height >= 1 and cell_pixel_height <= 4096 and
        @as(u32, columns) * cell_pixel_width <= std.math.maxInt(u16) and
        @as(u32, rows) * cell_pixel_height <= std.math.maxInt(u16);
}

fn validTerminalProfile(profile: TerminalProfileWire) bool {
    if ((!std.mem.eql(u8, profile.colorScheme, "dark") and
        !std.mem.eql(u8, profile.colorScheme, "light")) or
        profile.defaultBackgroundRgb > 0x00ffffff or
        profile.defaultForegroundRgb > 0x00ffffff or
        (profile.defaultCursorRgb != null and profile.defaultCursorRgb.? > 0x00ffffff) or
        profile.palette.len != 256 or
        profile.deviceAttributes.primaryFeatures.len > 64 or
        !validResponseText(profile.enquiryResponse) or
        !validResponseText(profile.versionResponse))
    {
        return false;
    }
    for (profile.palette) |color| if (color > 0x00ffffff) return false;
    for (profile.deviceAttributes.primaryFeatures, 0..) |feature, index| {
        for (profile.deviceAttributes.primaryFeatures[0..index]) |previous| {
            if (previous == feature) return false;
        }
    }
    return true;
}

fn validResponseText(value: []const u8) bool {
    return value.len <= 256 and !containsNul(value);
}

fn validDimension(value: u16) bool {
    return value >= 1 and value <= 4096;
}

fn containsNul(value: []const u8) bool {
    return std.mem.indexOfScalar(u8, value, 0) != null;
}

fn validSnapshotReason(value: []const u8) bool {
    const values = [_][]const u8{
        "base-mismatch",
        "decoder-rejected",
        "dimensions-mismatch",
        "generation-mismatch",
        "identity-mismatch",
        "sequence-gap",
        "store-reset",
    };
    for (values) |candidate| if (std.mem.eql(u8, value, candidate)) return true;
    return false;
}

fn parseCanonicalU64(value: []const u8) !u64 {
    if (value.len == 0 or (value.len > 1 and value[0] == '0')) return error.InvalidInteger;
    for (value) |byte| if (byte < '0' or byte > '9') return error.InvalidInteger;
    return std.fmt.parseInt(u64, value, 10);
}

fn parseRowOrigin(value: RowOriginWire) !terminal_history.Origin {
    if (std.mem.eql(u8, value.kind, "absolute")) {
        if (value.delta != null or value.offset != null or value.rowsBefore != null) {
            return error.InvalidOrigin;
        }
        return .{ .absolute = try parseCanonicalU64(value.row orelse return error.InvalidOrigin) };
    }
    if (std.mem.eql(u8, value.kind, "oldest")) {
        if (value.delta != null or value.row != null or value.rowsBefore != null) {
            return error.InvalidOrigin;
        }
        return .{ .oldest = value.offset orelse return error.InvalidOrigin };
    }
    if (std.mem.eql(u8, value.kind, "committed")) {
        if (value.offset != null or value.row != null or value.rowsBefore != null) {
            return error.InvalidOrigin;
        }
        return .{ .committed = value.delta orelse return error.InvalidOrigin };
    }
    if (std.mem.eql(u8, value.kind, "tail")) {
        if (value.delta != null or value.offset != null or value.row != null) {
            return error.InvalidOrigin;
        }
        return .{ .tail = value.rowsBefore orelse return error.InvalidOrigin };
    }
    return error.InvalidOrigin;
}

fn parseSelectionPoint(value: SelectionPointWire) !terminal_history.SelectionPoint {
    return .{
        .column = value.column,
        .row = try parseCanonicalU64(value.row),
    };
}

fn validExportCancelReason(value: []const u8) bool {
    inline for (.{ "clipboard-failed", "disposed", "replaced", "user" }) |candidate| {
        if (std.mem.eql(u8, value, candidate)) return true;
    }
    return false;
}

fn semanticAttachmentId(input: SemanticInput) core_module.Identifier {
    return switch (input) {
        inline else => |value| value.attachment_id,
    };
}

fn semanticReservationBytes(input: SemanticInput) !usize {
    return switch (input) {
        .focus => terminal_runtime.max_focus_encoded_bytes,
        .key => terminal_runtime.max_key_encoded_bytes,
        .mouse => terminal_runtime.max_mouse_encoded_bytes,
        .paste => |value| std.math.add(usize, value.text.len, 12),
        .text => |value| value.text.len,
    };
}

fn enqueueSemanticInput(
    runtime: *terminal_runtime.TerminalRuntime,
    input: SemanticInput,
) !usize {
    return switch (input) {
        .focus => |value| runtime.enqueueFocusInput(value.focused),
        .key => |value| runtime.enqueueKeyInput(.{
            .action = value.action,
            .composing = value.composing,
            .consumed_modifiers = value.consumed_modifiers,
            .key = value.key,
            .modifiers = value.modifiers,
            .text = value.text,
            .unshifted_codepoint = value.unshifted_codepoint,
        }),
        .mouse => |value| runtime.enqueueMouseInput(.{
            .action = value.action,
            .button = value.button,
            .modifiers = value.modifiers,
            .pressed_buttons = value.pressed_buttons,
            .x = value.x,
            .y = value.y,
        }),
        .paste => |value| runtime.enqueuePasteInput(value.text),
        .text => |value| runtime.enqueueTextInput(value.text),
    };
}

fn launchDigest(allocator: std.mem.Allocator, launch: LaunchWire) !core_module.LaunchDigest {
    var hash = std.crypto.hash.sha2.Sha256.init(.{});
    hash.update("orchbee-terminal-launch-v2");
    hashField(&hash, launch.executable);
    hashCount(&hash, launch.arguments.len);
    for (launch.arguments) |argument| hashField(&hash, argument);
    if (launch.currentDirectory) |directory| {
        hash.update(&.{1});
        hashField(&hash, directory);
    } else hash.update(&.{0});
    var dimensions: [4]u8 = undefined;
    std.mem.writeInt(u16, dimensions[0..2], launch.columns, .little);
    std.mem.writeInt(u16, dimensions[2..4], launch.rows, .little);
    hash.update(&dimensions);
    hashOptionalU16(&hash, launch.cellPixelWidth);
    hashOptionalU16(&hash, launch.cellPixelHeight);

    hashCount(&hash, launch.environment.len);
    const indices = try allocator.alloc(usize, launch.environment.len);
    defer allocator.free(indices);
    for (indices, 0..) |*index, value| index.* = value;
    std.mem.sort(usize, indices, launch.environment, struct {
        fn lessThan(environment: []const EnvironmentWire, left: usize, right: usize) bool {
            return std.mem.lessThan(u8, environment[left].name, environment[right].name);
        }
    }.lessThan);
    for (indices) |index| {
        const entry = launch.environment[index];
        hashField(&hash, entry.name);
        if (entry.value) |value| {
            hash.update(&.{1});
            hashField(&hash, value);
        } else hash.update(&.{0});
    }
    if (launch.terminalProfile) |profile| {
        hash.update(&.{1});
        hashField(&hash, profile.colorScheme);
        hashInt(&hash, u32, profile.defaultBackgroundRgb);
        if (profile.defaultCursorRgb) |cursor| {
            hash.update(&.{1});
            hashInt(&hash, u32, cursor);
        } else hash.update(&.{0});
        hashInt(&hash, u32, profile.defaultForegroundRgb);
        hashInt(&hash, u16, profile.deviceAttributes.primaryConformanceLevel);
        hashCount(&hash, profile.deviceAttributes.primaryFeatures.len);
        for (profile.deviceAttributes.primaryFeatures) |feature| hashInt(&hash, u16, feature);
        hashInt(&hash, u16, profile.deviceAttributes.secondaryDeviceType);
        hashInt(&hash, u16, profile.deviceAttributes.secondaryFirmwareVersion);
        hashInt(&hash, u16, profile.deviceAttributes.secondaryRomCartridge);
        hashInt(&hash, u32, profile.deviceAttributes.tertiaryUnitId);
        hashField(&hash, profile.enquiryResponse);
        hashCount(&hash, profile.palette.len);
        for (profile.palette) |color| hashInt(&hash, u32, color);
        hashField(&hash, profile.versionResponse);
    } else hash.update(&.{0});
    var digest: core_module.LaunchDigest = undefined;
    hash.final(&digest);
    return digest;
}

fn hashOptionalU16(hash: *std.crypto.hash.sha2.Sha256, value: ?u16) void {
    if (value) |integer| {
        hash.update(&.{1});
        hashInt(hash, u16, integer);
    } else hash.update(&.{0});
}

fn hashInt(
    hash: *std.crypto.hash.sha2.Sha256,
    comptime Integer: type,
    value: Integer,
) void {
    var bytes: [@sizeOf(Integer)]u8 = undefined;
    std.mem.writeInt(Integer, &bytes, value, .little);
    hash.update(&bytes);
}

fn hashCount(hash: *std.crypto.hash.sha2.Sha256, value: usize) void {
    var bytes: [8]u8 = undefined;
    std.mem.writeInt(u64, &bytes, value, .little);
    hash.update(&bytes);
}

fn hashField(hash: *std.crypto.hash.sha2.Sha256, value: []const u8) void {
    var length: [8]u8 = undefined;
    std.mem.writeInt(u64, &length, value.len, .little);
    hash.update(&length);
    hash.update(value);
}

test "launch digest is unambiguous and environment-order independent" {
    const environment_a = [_]EnvironmentWire{
        .{ .name = "B", .value = null },
        .{ .name = "A", .value = "" },
    };
    const environment_b = [_]EnvironmentWire{
        .{ .name = "A", .value = "" },
        .{ .name = "B", .value = null },
    };
    const empty_argument = [_][]const u8{""};
    const base: LaunchWire = .{
        .arguments = &.{},
        .columns = 80,
        .currentDirectory = null,
        .environment = &environment_a,
        .executable = "shell",
        .rows = 24,
    };
    var reordered = base;
    reordered.environment = &environment_b;
    var with_argument = base;
    with_argument.arguments = &empty_argument;

    const base_digest = try launchDigest(std.testing.allocator, base);
    const reordered_digest = try launchDigest(std.testing.allocator, reordered);
    const argument_digest = try launchDigest(std.testing.allocator, with_argument);
    try std.testing.expectEqualSlices(u8, &base_digest, &reordered_digest);
    try std.testing.expect(!std.mem.eql(u8, &base_digest, &argument_digest));
}

fn parseIdentifier(value: []const u8) !core_module.Identifier {
    if (value.len != 32) return error.InvalidIdentifier;
    var result: core_module.Identifier = undefined;
    var non_zero = false;
    for (&result, 0..) |*byte, index| {
        const high = std.fmt.charToDigit(value[index * 2], 16) catch return error.InvalidIdentifier;
        const low = std.fmt.charToDigit(value[index * 2 + 1], 16) catch return error.InvalidIdentifier;
        if ((value[index * 2] >= 'A' and value[index * 2] <= 'F') or
            (value[index * 2 + 1] >= 'A' and value[index * 2 + 1] <= 'F'))
        {
            return error.InvalidIdentifier;
        }
        byte.* = @intCast(high * 16 + low);
        non_zero = non_zero or byte.* != 0;
    }
    if (!non_zero) return error.InvalidIdentifier;
    return result;
}

fn identifierText(identifier: core_module.Identifier, buffer: *[32]u8) []const u8 {
    const digits = "0123456789abcdef";
    for (identifier, 0..) |byte, index| {
        buffer[index * 2] = digits[byte >> 4];
        buffer[index * 2 + 1] = digits[byte & 0x0f];
    }
    return buffer;
}

fn identifierEqual(left: core_module.Identifier, right: core_module.Identifier) bool {
    return std.mem.eql(u8, &left, &right);
}

fn randomIdentifier() core_module.Identifier {
    while (true) {
        var identifier: core_module.Identifier = undefined;
        std.crypto.random.bytes(&identifier);
        for (identifier) |byte| {
            if (byte != 0) return identifier;
        }
    }
}

fn terminalAcknowledgedJson(allocator: std.mem.Allocator, terminal_id: core_module.Identifier) ![]u8 {
    var buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &buffer)},
    );
}

fn exitResultJson(allocator: std.mem.Allocator, result: ?core_module.ExitResult) ![]u8 {
    const value = result orelse return allocator.dupe(u8, "null");
    return switch (value) {
        .exit_code => |code| std.fmt.allocPrint(allocator, "{{\"code\":{d},\"kind\":\"exit-code\"}}", .{code}),
        .signal => |signal| std.fmt.allocPrint(allocator, "{{\"kind\":\"signal\",\"signal\":{d}}}", .{signal}),
        .unknown => |unknown| std.fmt.allocPrint(allocator, "{{\"kind\":\"unknown\",\"value\":{d}}}", .{unknown}),
    };
}

fn coreExitResult(result: pty_process.ExitResult) core_module.ExitResult {
    return switch (result) {
        .exit_code => |code| .{ .exit_code = code },
        .signal => |signal| .{ .signal = signal },
        .unknown => |unknown| .{ .unknown = unknown },
    };
}

fn protocolErrorCode(err: anyerror) []const u8 {
    return switch (err) {
        error.InvalidField => "invalid-field",
        error.InvalidMagic => "invalid-magic",
        error.InvalidUtf8 => "invalid-utf8",
        error.LengthMismatch => "length-mismatch",
        error.LimitExceeded => "limit-exceeded",
        error.Truncated => "truncated",
        error.UnknownRequiredFlag => "unknown-required-flag",
        error.UnknownRequiredMessage => "unknown-required-message",
        error.UnsupportedVersion => "unsupported-version",
        else => "protocol-error",
    };
}

fn coreError(err: anyerror) struct { code: []const u8, message: []const u8, retryable: bool } {
    return switch (err) {
        error.AttachmentConflict => .{ .code = "attachment-conflict", .message = "AttachmentId conflicts with retained state", .retryable = false },
        error.ConnectionConflict => .{ .code = "connection-conflict", .message = "physical connection identity conflicts with retained state", .retryable = false },
        error.CreationConflict => .{ .code = "creation-conflict", .message = "CreationId has a different launch", .retryable = false },
        error.InputBackpressure => .{ .code = "input-backpressure", .message = "terminal input queue is full", .retryable = true },
        error.InvalidFrameAcknowledgement => .{ .code = "invalid-frame-acknowledgement", .message = "frame acknowledgement does not match the in-flight projection", .retryable = false },
        error.InvalidPhase => .{ .code = "invalid-phase", .message = "operation is invalid in the terminal phase", .retryable = false },
        error.InventoryGenerationOverflow => .{ .code = "resource-exhausted", .message = "terminal inventory generation space is exhausted", .retryable = false },
        error.MissingConnection => .{ .code = "missing-connection", .message = "client connection is not active", .retryable = false },
        error.MissingResource => .{ .code = "missing-resource", .message = "terminal or attachment is missing", .retryable = false },
        error.RevisionOverflow => .{ .code = "resource-exhausted", .message = "terminal revision space is exhausted", .retryable = false },
        error.ResizeLeaseDenied => .{ .code = "resize-lease-denied", .message = "attachment does not own the resize lease", .retryable = false },
        error.SequenceOverflow => .{ .code = "resource-exhausted", .message = "attachment frame sequence space is exhausted", .retryable = false },
        error.TerminalConflict => .{ .code = "terminal-conflict", .message = "TerminalId conflicts with retained state", .retryable = true },
        error.OutOfMemory => .{ .code = "resource-exhausted", .message = "Host could not retain bounded state", .retryable = true },
        else => .{ .code = "internal-error", .message = "Host operation failed", .retryable = true },
    };
}

test "handshake binds client and negotiates input bytes" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{};
    const payload =
        \\{"clientId":"22222222222222222222222222222222","frameBytes":1024,"inputBytes":2048,"protocol":{"major":1,"maximumMinor":0,"minimumMinor":0},"rowWindowBytes":4096,"supportedOptionalFeatures":[]}
    ;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records.items.len);
    const response = try protocol.validateTransportEnvelope(outgoing.records.items[0]);
    try std.testing.expectEqual(MessageKind.hello_accepted, response.kind);
    try std.testing.expect(connection.client_id != null);
    try std.testing.expectEqual(@as(usize, 2048), connection.input_bytes);
    service.disconnect(&connection);
}

test "idle stop requires both the final disconnect and an empty terminal registry" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    try std.testing.expect(service.canStopWhenIdle());

    const connection_id: core_module.ConnectionId = 7;
    _ = try service.core.connect(connection_id, [_]u8{0x21} ** 16);
    try std.testing.expect(!service.canStopWhenIdle());
    _ = try service.core.createTerminal(
        connection_id,
        [_]u8{0x22} ** 16,
        [_]u8{0x23} ** 16,
        [_]u8{0x24} ** 32,
        80,
        24,
    );
    _ = service.core.disconnect(connection_id);
    try std.testing.expect(!service.canStopWhenIdle());

    _ = try service.core.spawnFailed([_]u8{0x23} ** 16);
    try std.testing.expect(service.canStopWhenIdle());
}

test "equal wire ClientIds receive distinct physical connection identities" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var first_connection: ConnectionState = .{};
    var second_connection: ConnectionState = .{};
    defer service.disconnect(&first_connection);
    defer service.disconnect(&second_connection);

    try testHandshakeV1_3(&service, &first_connection, "22222222222222222222222222222222");
    try testHandshakeV1_3(&service, &second_connection, "22222222222222222222222222222222");
    try std.testing.expect(first_connection.connection_id != second_connection.connection_id);
    try std.testing.expectEqual(@as(usize, 2), service.core.connections.count());
    try std.testing.expectEqualSlices(
        u8,
        &first_connection.client_id.?,
        &second_connection.client_id.?,
    );

    service.disconnect(&first_connection);
    try std.testing.expectEqual(@as(usize, 1), service.core.connections.count());
    try std.testing.expect(
        service.core.connectionClientId(second_connection.connection_id.?) != null,
    );
}

test "protocol 1.3 pages a stable inventory and rejects stale cursors" {
    const TestInventoryPage = struct {
        inventoryGeneration: []const u8,
        nextCursor: ?InventoryCursorWire,
        terminals: []std.json.Value,
    };

    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{};
    var legacy_connection: ConnectionState = .{};
    defer service.disconnect(&connection);
    defer service.disconnect(&legacy_connection);
    try testHandshakeV1_3(&service, &connection, "22222222222222222222222222222222");

    const replay_creation_id = testIdentifier(0x31, 0);
    const replay_terminal_id = testIdentifier(0x32, 0);
    var replay_creation_buffer: [32]u8 = undefined;
    const replay_payload = try testCreatePayloadV1_3(
        identifierText(replay_creation_id, &replay_creation_buffer),
        "orchbee-terminal-host-deliberately-missing-executable",
        &.{},
    );
    defer std.testing.allocator.free(replay_payload);
    const replay_request = try std.json.parseFromSlice(
        CreateWire,
        std.testing.allocator,
        replay_payload,
        .{},
    );
    defer replay_request.deinit();
    _ = try service.core.createTerminal(
        connection.connection_id.?,
        replay_creation_id,
        replay_terminal_id,
        try launchDigest(std.testing.allocator, replay_request.value.launch),
        80,
        24,
    );
    for (1..130) |index| {
        const creation_id = testIdentifier(0x31, index);
        const terminal_id = testIdentifier(0x32, index);
        _ = try service.core.createTerminal(
            connection.connection_id.?,
            creation_id,
            terminal_id,
            [_]u8{@intCast(index + 1)} ** 32,
            80,
            24,
        );
    }

    var first_response = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        "{\"cursor\":null,\"maximumTerminals\":128}",
        [_]u8{0x41} ** 16,
    );
    defer first_response.deinit();
    try std.testing.expectEqual(@as(usize, 1), first_response.records.items.len);
    const first_envelope = try protocol.validateTransportEnvelope(first_response.records.items[0]);
    const first_page = try std.json.parseFromSlice(
        TestInventoryPage,
        std.testing.allocator,
        first_envelope.payload,
        .{},
    );
    defer first_page.deinit();
    try std.testing.expectEqual(@as(usize, 128), first_page.value.terminals.len);
    try std.testing.expect(first_page.value.nextCursor != null);
    try std.testing.expectEqualStrings("128", first_page.value.nextCursor.?.offset);

    const second_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"cursor\":{{\"inventoryGeneration\":\"{s}\",\"offset\":\"{s}\"}},\"maximumTerminals\":128}}",
        .{
            first_page.value.nextCursor.?.inventoryGeneration,
            first_page.value.nextCursor.?.offset,
        },
    );
    defer std.testing.allocator.free(second_payload);
    var second_response = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        second_payload,
        [_]u8{0x42} ** 16,
    );
    defer second_response.deinit();
    const second_envelope = try protocol.validateTransportEnvelope(second_response.records.items[0]);
    const second_page = try std.json.parseFromSlice(
        TestInventoryPage,
        std.testing.allocator,
        second_envelope.payload,
        .{},
    );
    defer second_page.deinit();
    try std.testing.expectEqual(@as(usize, 2), second_page.value.terminals.len);
    try std.testing.expect(second_page.value.nextCursor == null);
    try std.testing.expectEqualStrings(
        first_page.value.inventoryGeneration,
        second_page.value.inventoryGeneration,
    );

    _ = try service.core.createTerminal(
        connection.connection_id.?,
        testIdentifier(0x31, 130),
        testIdentifier(0x32, 130),
        [_]u8{0xee} ** 32,
        80,
        24,
    );
    var stale_response = try testRequest(
        &service,
        &connection,
        MessageKind.list_terminals,
        .control_json,
        second_payload,
        [_]u8{0x43} ** 16,
    );
    defer stale_response.deinit();
    const stale_envelope = try protocol.validateTransportEnvelope(stale_response.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, stale_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, stale_envelope.payload, "stale-inventory-cursor") != null);

    try testHandshakeV1_2(
        &service,
        &legacy_connection,
        "23232323232323232323232323232323",
    );
    var legacy_response = try testRequest(
        &service,
        &legacy_connection,
        MessageKind.list_terminals,
        .control_json,
        "{}",
        [_]u8{0x44} ** 16,
    );
    defer legacy_response.deinit();
    const legacy_envelope = try protocol.validateTransportEnvelope(legacy_response.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, legacy_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, legacy_envelope.payload, "protocol-upgrade-required") != null);

    var replay_response = try testRequest(
        &service,
        &legacy_connection,
        MessageKind.create_terminal,
        .control_json,
        replay_payload,
        [_]u8{0x45} ** 16,
    );
    defer replay_response.deinit();
    try std.testing.expectEqual(
        MessageKind.create_terminal,
        (try protocol.validateTransportEnvelope(replay_response.records.items[0])).kind,
    );

    const distinct_payload = try testCreatePayloadV1_3(
        "abababababababababababababababab",
        "orchbee-terminal-host-deliberately-missing-executable",
        &.{},
    );
    defer std.testing.allocator.free(distinct_payload);
    var distinct_response = try testRequest(
        &service,
        &legacy_connection,
        MessageKind.create_terminal,
        .control_json,
        distinct_payload,
        [_]u8{0x46} ** 16,
    );
    defer distinct_response.deinit();
    const distinct_envelope = try protocol.validateTransportEnvelope(distinct_response.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, distinct_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, distinct_envelope.payload, "protocol-upgrade-required") != null);
}

test "protocol 1.3 rejects force termination before parsing its payload" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{};
    defer service.disconnect(&connection);
    try testHandshakeV1_3(&service, &connection, "22222222222222222222222222222222");

    var response = try testRequest(
        &service,
        &connection,
        MessageKind.force_terminate_terminal,
        .control_json,
        "{}",
        [_]u8{0x41} ** 16,
    );
    defer response.deinit();
    try std.testing.expectEqual(@as(usize, 1), response.records.items.len);
    const envelope = try protocol.validateTransportEnvelope(response.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, envelope.payload, "unsupported-version") != null);
}

test "protocol 1.4 force-terminates every retained POSIX session member" {
    if (@import("builtin").os.tag == .windows) return error.SkipZigTest;
    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .{};
    defer {
        for (service.runtimes.values()) |entry| entry.runtime.forceTerminate() catch {};
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshakeV1_4(&service, &connection, "22222222222222222222222222222222");

    const arguments = [_][]const u8{
        "-c",
        "trap '' HUP; set -m; printf ready; while :; do sleep 600 & wait; done",
    };
    const create_payload = try testCreatePayloadV1_3(
        "33333333333333333333333333333333",
        "/bin/sh",
        &arguments,
    );
    defer std.testing.allocator.free(create_payload);
    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x42} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.core.terminals.keys()[0];
    var ready = false;
    for (0..400) |_| {
        const plain = try service.findRuntime(terminal_id).?.plainString(std.testing.allocator);
        defer std.testing.allocator.free(plain);
        if (std.mem.indexOf(u8, plain, "ready") != null) {
            ready = true;
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expect(ready);
    const target = try testTerminalTargetPayload(terminal_id);
    defer std.testing.allocator.free(target);

    var premature = try testRequest(
        &service,
        &connection,
        MessageKind.force_terminate_terminal,
        .control_json,
        target,
        [_]u8{0x43} ** 16,
    );
    defer premature.deinit();
    try std.testing.expectEqual(
        MessageKind.typed_failure,
        (try protocol.validateTransportEnvelope(premature.records.items[0])).kind,
    );

    var graceful = try testRequest(
        &service,
        &connection,
        MessageKind.terminate_terminal,
        .control_json,
        target,
        [_]u8{0x44} ** 16,
    );
    defer graceful.deinit();
    try std.testing.expectEqual(
        MessageKind.terminate_terminal,
        (try protocol.validateTransportEnvelope(graceful.records.items[0])).kind,
    );

    var forced = try testRequest(
        &service,
        &connection,
        MessageKind.force_terminate_terminal,
        .control_json,
        target,
        [_]u8{0x45} ** 16,
    );
    defer forced.deinit();
    try std.testing.expectEqual(
        MessageKind.force_terminate_terminal,
        (try protocol.validateTransportEnvelope(forced.records.items[0])).kind,
    );
    try std.testing.expect(service.core.findTerminal(terminal_id).?.force_terminate_requested);

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.core.findTerminal(terminal_id).?.phase == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.core.findTerminal(terminal_id).?.phase,
    );
}

test "malformed request receives a correlated typed failure without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{
        .connection_id = 1,
        .client_id = [_]u8{0x22} ** 16,
        .frame_bytes = 1024,
        .input_bytes = 1024,
        .row_window_bytes = 1024,
    };
    _ = try service.core.connect(connection.connection_id.?, connection.client_id.?);
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.create_terminal,
        .control_json,
        "{",
        correlation,
        service.core.epoch,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records.items.len);
    const response = try protocol.validateTransportEnvelope(outgoing.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, response.kind);
    try std.testing.expectEqualSlices(u8, &correlation, &response.correlation_id.?);
    try std.testing.expectEqual(@as(usize, 0), service.core.terminalCount());
    service.disconnect(&connection);
}

test "stale HostEpoch receives a correlated failure without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{
        .connection_id = 1,
        .client_id = [_]u8{0x22} ** 16,
        .frame_bytes = 1024,
        .input_bytes = 1024,
        .row_window_bytes = 1024,
    };
    _ = try service.core.connect(connection.connection_id.?, connection.client_id.?);
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.list_terminals,
        .control_json,
        "{}",
        correlation,
        [_]u8{0x44} ** 16,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records.items.len);
    const response = try protocol.validateTransportEnvelope(outgoing.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, response.kind);
    try std.testing.expectEqualSlices(u8, &correlation, &response.correlation_id.?);
    try std.testing.expect(std.mem.indexOf(u8, response.payload, "stale-epoch") != null);
    try std.testing.expectEqual(@as(usize, 0), service.core.terminalCount());
    service.disconnect(&connection);
}

test "unknown optional request is skipped without state mutation" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{
        .connection_id = 1,
        .client_id = [_]u8{0x22} ** 16,
        .frame_bytes = 1024,
        .input_bytes = 1024,
        .row_window_bytes = 1024,
    };
    _ = try service.core.connect(connection.connection_id.?, connection.client_id.?);
    const correlation = [_]u8{0x33} ** 16;
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        200,
        .control_json,
        "{}",
        correlation,
        service.core.epoch,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    std.mem.writeInt(u16, record[6..8], 1, .little);

    var outgoing = try service.handle(&connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 0), outgoing.records.items.len);
    try std.testing.expectEqual(@as(usize, 0), service.core.terminalCount());
    service.disconnect(&connection);
}

test "invalid transport framing closes dispatch without a response" {
    var service = HostService.init(std.testing.allocator);
    defer service.deinit();
    var connection: ConnectionState = .{};
    var record = [_]u8{0} ** protocol.transport_envelope_header_bytes;
    @memcpy(record[0..4], "NOPE");
    try std.testing.expectError(
        error.InvalidTransportRecord,
        service.handle(&connection, &record),
    );
}

test "real PTY lifecycle survives disconnect and an idempotent create retry" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf 'ready:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =ready:& set /p value=& <nul set /p =!value!",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const environment = [_]EnvironmentWire{};
    var create_writer: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer create_writer.deinit();
    try std.json.Stringify.value(.{
        .creationId = "33333333333333333333333333333333",
        .launch = .{
            .arguments = arguments,
            .columns = 80,
            .currentDirectory = @as(?[]const u8, null),
            .environment = &environment,
            .executable = executable,
            .rows = 24,
        },
    }, .{}, &create_writer.writer);
    const create_payload = try create_writer.toOwnedSlice();
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var first_connection: ConnectionState = .{};
    var second_connection: ConnectionState = .{};
    defer {
        service.disconnect(&first_connection);
        service.disconnect(&second_connection);
        service.deinit();
    }

    try testHandshake(&service, &first_connection, "22222222222222222222222222222222");
    const create_correlation = [_]u8{0x51} ** 16;
    var created = try testRequest(
        &service,
        &first_connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        create_correlation,
    );
    defer created.deinit();
    try std.testing.expectEqual(@as(usize, 2), created.records.items.len);
    try std.testing.expectEqual(MessageKind.create_terminal, (try protocol.validateTransportEnvelope(created.records.items[0])).kind);
    try std.testing.expectEqual(MessageKind.terminal_started, (try protocol.validateTransportEnvelope(created.records.items[1])).kind);
    const terminal_id = service.core.terminals.keys()[0];
    const retained_runtime = service.findRuntime(terminal_id).?;

    service.disconnect(&first_connection);
    try std.testing.expect(service.core.findTerminal(terminal_id) != null);
    try std.testing.expect(service.findRuntime(terminal_id) == retained_runtime);
    try std.testing.expect(!service.core.findTerminal(terminal_id).?.terminate_requested);

    try testHandshake(&service, &second_connection, "66666666666666666666666666666666");
    var retried = try testRequest(
        &service,
        &second_connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x52} ** 16,
    );
    defer retried.deinit();
    try std.testing.expectEqual(@as(usize, 1), retried.records.items.len);
    try std.testing.expect(service.core.findTerminal(terminal_id) != null);
    try std.testing.expect(service.findRuntime(terminal_id) == retained_runtime);

    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"44444444444444444444444444444444\",\"capabilities\":{{\"input\":true,\"read\":false,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &second_connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x53} ** 16,
    );
    defer attached.deinit();
    try std.testing.expectEqual(MessageKind.attachment_capabilities, (try protocol.validateTransportEnvelope(attached.records.items[0])).kind);

    const resize_payload =
        \\{"attachmentId":"44444444444444444444444444444444","columns":100,"rows":30}
    ;
    var resized = try testRequest(
        &service,
        &second_connection,
        MessageKind.resize_terminal,
        .control_json,
        resize_payload,
        [_]u8{0x54} ** 16,
    );
    defer resized.deinit();
    try std.testing.expectEqual(MessageKind.resize_accepted, (try protocol.validateTransportEnvelope(resized.records.items[0])).kind);
    try std.testing.expectEqual(@as(u16, 100), service.core.findTerminal(terminal_id).?.columns);
    try std.testing.expectEqual(@as(u16, 30), service.core.findTerminal(terminal_id).?.rows);

    const input_text: []const u8 = if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n";
    var input_payload: [16 + input_text.len]u8 = undefined;
    @memset(input_payload[0..16], 0x44);
    @memcpy(input_payload[16..], input_text);
    var input = try testRequest(
        &service,
        &second_connection,
        MessageKind.write_input,
        .raw_bytes,
        &input_payload,
        [_]u8{0x55} ** 16,
    );
    defer input.deinit();
    try std.testing.expectEqual(MessageKind.input_accepted, (try protocol.validateTransportEnvelope(input.records.items[0])).kind);

    var observed_exit_event = false;
    for (0..400) |_| {
        var events = try service.pollEvents(&second_connection);
        for (events.records.items) |event_record| {
            const event = try protocol.validateTransportEnvelope(event_record);
            observed_exit_event = observed_exit_event or event.kind == MessageKind.terminal_exited;
        }
        events.deinit();
        if (service.core.findTerminal(terminal_id).?.phase == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(core_module.TerminalPhase.exited, service.core.findTerminal(terminal_id).?.phase);
    try std.testing.expect(observed_exit_event);
    try std.testing.expect(service.core.findTerminal(terminal_id).?.revision >= 2);

    const plain = try service.findRuntime(terminal_id).?.plainString(std.testing.allocator);
    defer std.testing.allocator.free(plain);
    try std.testing.expect(std.mem.indexOf(u8, plain, "ready:orchbee") != null);

    var dispose_buffer: [32]u8 = undefined;
    const dispose_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &dispose_buffer)},
    );
    defer std.testing.allocator.free(dispose_payload);
    var disposed = try testRequest(
        &service,
        &second_connection,
        MessageKind.dispose_terminal,
        .control_json,
        dispose_payload,
        [_]u8{0x56} ** 16,
    );
    defer disposed.deinit();
    try std.testing.expect(service.findRuntime(terminal_id) == null);
    try std.testing.expect(service.core.findTerminal(terminal_id) == null);
}

test "two PTY runtimes route input independently and isolate spawn failure" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const first_posix_arguments = [_][]const u8{
        "-c",
        "printf 'first-ready:'; read value; printf 'first-%s' \"$value\"",
    };
    const second_posix_arguments = [_][]const u8{
        "-c",
        "printf 'second-ready:'; read value; printf 'second-%s' \"$value\"",
    };
    const first_windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =first-ready:& set /p value=& <nul set /p =first-!value!",
    };
    const second_windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =second-ready:& set /p value=& <nul set /p =second-!value!",
    };
    const first_arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &first_windows_arguments
    else
        &first_posix_arguments;
    const second_arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &second_windows_arguments
    else
        &second_posix_arguments;

    const first_create_payload = try testCreatePayloadV1_3(
        "31313131313131313131313131313131",
        executable,
        first_arguments,
    );
    defer std.testing.allocator.free(first_create_payload);
    const failed_create_payload = try testCreatePayloadV1_3(
        "32323232323232323232323232323232",
        "orchbee-terminal-host-deliberately-missing-executable",
        &.{},
    );
    defer std.testing.allocator.free(failed_create_payload);
    const second_create_payload = try testCreatePayloadV1_3(
        "33333333333333333333333333333333",
        executable,
        second_arguments,
    );
    defer std.testing.allocator.free(second_create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .{};
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshakeV1_3(&service, &connection, "22222222222222222222222222222222");

    var first_created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        first_create_payload,
        [_]u8{0x51} ** 16,
    );
    defer first_created.deinit();
    try std.testing.expectEqual(@as(usize, 1), service.core.terminalCount());
    try std.testing.expectEqual(@as(usize, 1), service.runtimes.count());
    const first_terminal_id = service.core.terminals.keys()[0];

    var failed_create = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        failed_create_payload,
        [_]u8{0x52} ** 16,
    );
    defer failed_create.deinit();
    const failed_envelope = try protocol.validateTransportEnvelope(failed_create.records.items[0]);
    try std.testing.expectEqual(MessageKind.typed_failure, failed_envelope.kind);
    try std.testing.expect(std.mem.indexOf(u8, failed_envelope.payload, "spawn-failed") != null);
    try std.testing.expectEqual(@as(usize, 1), service.core.terminalCount());
    try std.testing.expectEqual(@as(usize, 1), service.runtimes.count());
    try std.testing.expect(service.core.findTerminal(first_terminal_id) != null);

    var second_created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        second_create_payload,
        [_]u8{0x53} ** 16,
    );
    defer second_created.deinit();
    try std.testing.expectEqual(@as(usize, 2), service.core.terminalCount());
    try std.testing.expectEqual(@as(usize, 2), service.runtimes.count());
    const second_terminal_id = service.core.terminals.keys()[1];

    const first_attachment_id = [_]u8{0x44} ** 16;
    const second_attachment_id = [_]u8{0x55} ** 16;
    const first_attach_payload = try testAttachPayload(first_attachment_id, first_terminal_id);
    defer std.testing.allocator.free(first_attach_payload);
    const second_attach_payload = try testAttachPayload(second_attachment_id, second_terminal_id);
    defer std.testing.allocator.free(second_attach_payload);
    var first_attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        first_attach_payload,
        [_]u8{0x54} ** 16,
    );
    defer first_attached.deinit();
    var second_attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        second_attach_payload,
        [_]u8{0x55} ** 16,
    );
    defer second_attached.deinit();

    const first_input: []const u8 = if (builtin.os.tag == .windows) "alpha\r\n" else "alpha\n";
    const second_input: []const u8 = if (builtin.os.tag == .windows) "beta\r\n" else "beta\n";
    const first_input_payload = try testRawInputPayload(first_attachment_id, first_input);
    defer std.testing.allocator.free(first_input_payload);
    const second_input_payload = try testRawInputPayload(second_attachment_id, second_input);
    defer std.testing.allocator.free(second_input_payload);
    var first_accepted = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        first_input_payload,
        [_]u8{0x56} ** 16,
    );
    defer first_accepted.deinit();
    var second_accepted = try testRequest(
        &service,
        &connection,
        MessageKind.write_input,
        .raw_bytes,
        second_input_payload,
        [_]u8{0x57} ** 16,
    );
    defer second_accepted.deinit();

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.core.findTerminal(first_terminal_id).?.phase == .exited and
            service.core.findTerminal(second_terminal_id).?.phase == .exited)
        {
            break;
        }
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.core.findTerminal(first_terminal_id).?.phase,
    );
    try std.testing.expectEqual(
        core_module.TerminalPhase.exited,
        service.core.findTerminal(second_terminal_id).?.phase,
    );
    const first_plain = try service.findRuntime(first_terminal_id).?.plainString(std.testing.allocator);
    defer std.testing.allocator.free(first_plain);
    const second_plain = try service.findRuntime(second_terminal_id).?.plainString(std.testing.allocator);
    defer std.testing.allocator.free(second_plain);
    try std.testing.expect(std.mem.indexOf(u8, first_plain, "first-alpha") != null);
    try std.testing.expect(std.mem.indexOf(u8, first_plain, "second-beta") == null);
    try std.testing.expect(std.mem.indexOf(u8, second_plain, "second-beta") != null);
    try std.testing.expect(std.mem.indexOf(u8, second_plain, "first-alpha") == null);

    const first_dispose_payload = try testTerminalTargetPayload(first_terminal_id);
    defer std.testing.allocator.free(first_dispose_payload);
    var first_disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        first_dispose_payload,
        [_]u8{0x58} ** 16,
    );
    defer first_disposed.deinit();
    try std.testing.expect(service.core.findTerminal(first_terminal_id) == null);
    try std.testing.expect(service.findRuntime(first_terminal_id) == null);
    try std.testing.expect(service.core.findTerminal(second_terminal_id) != null);
    try std.testing.expect(service.findRuntime(second_terminal_id) != null);

    const second_dispose_payload = try testTerminalTargetPayload(second_terminal_id);
    defer std.testing.allocator.free(second_dispose_payload);
    var second_disposed = try testRequest(
        &service,
        &connection,
        MessageKind.dispose_terminal,
        .control_json,
        second_dispose_payload,
        [_]u8{0x59} ** 16,
    );
    defer second_disposed.deinit();
    try std.testing.expectEqual(@as(usize, 0), service.core.terminalCount());
    try std.testing.expectEqual(@as(usize, 0), service.runtimes.count());
}

test "protocol 1.1 activates a lazy live projection and accepts semantic text" {
    const builtin = @import("builtin");
    const windows_executable: ?[]u8 = if (builtin.os.tag == .windows)
        try std.process.getEnvVarOwned(std.testing.allocator, "ComSpec")
    else
        null;
    defer if (windows_executable) |value| std.testing.allocator.free(value);
    const executable = if (windows_executable) |value| value else "/bin/sh";
    const posix_arguments = [_][]const u8{
        "-c",
        "printf 'ready:'; read value; printf '%s' \"$value\"",
    };
    const windows_arguments = [_][]const u8{
        "/Q",
        "/D",
        "/V:ON",
        "/S",
        "/C",
        "<nul set /p =ready:& set /p value=& <nul set /p =!value!",
    };
    const arguments: []const []const u8 = if (builtin.os.tag == .windows)
        &windows_arguments
    else
        &posix_arguments;
    const environment = [_]EnvironmentWire{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    var create_writer: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer create_writer.deinit();
    try std.json.Stringify.value(.{
        .creationId = "33333333333333333333333333333333",
        .launch = .{
            .arguments = arguments,
            .cellPixelHeight = 16,
            .cellPixelWidth = 8,
            .columns = 20,
            .currentDirectory = @as(?[]const u8, null),
            .environment = &environment,
            .executable = executable,
            .rows = 4,
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
                .versionResponse = "Orchbee test",
            },
        },
    }, .{}, &create_writer.writer);
    const create_payload = try create_writer.toOwnedSlice();
    defer std.testing.allocator.free(create_payload);

    var service = HostService.init(std.testing.allocator);
    var connection: ConnectionState = .{};
    defer {
        service.disconnect(&connection);
        service.deinit();
    }
    try testHandshakeV1_1(&service, &connection, "22222222222222222222222222222222");
    try std.testing.expectEqual(@as(u16, 1), connection.protocol_minor);

    var created = try testRequest(
        &service,
        &connection,
        MessageKind.create_terminal,
        .control_json,
        create_payload,
        [_]u8{0x61} ** 16,
    );
    defer created.deinit();
    const terminal_id = service.core.terminals.keys()[0];
    const runtime = service.findRuntime(terminal_id).?;
    try std.testing.expectEqual(@as(u64, 0), runtime.projectionCounters().render_state_creations);
    var terminal_buffer: [32]u8 = undefined;
    const attach_payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"44444444444444444444444444444444\",\"capabilities\":{{\"input\":true,\"read\":true,\"resize\":true}},\"requestResizeLease\":true,\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
    defer std.testing.allocator.free(attach_payload);
    var attached = try testRequest(
        &service,
        &connection,
        MessageKind.attach_terminal,
        .control_json,
        attach_payload,
        [_]u8{0x62} ** 16,
    );
    defer attached.deinit();
    try std.testing.expectEqual(MessageKind.attachment_capabilities, (try protocol.validateTransportEnvelope(attached.records.items[0])).kind);
    try std.testing.expectEqual(@as(u64, 0), runtime.projectionCounters().render_state_creations);

    const demand_payload =
        \\{"attachmentId":"44444444444444444444444444444444","demand":"active"}
    ;
    var demanded = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        demand_payload,
        [_]u8{0x63} ** 16,
    );
    defer demanded.deinit();
    try std.testing.expectEqual(@as(usize, 2), demanded.records.items.len);
    try std.testing.expectEqual(MessageKind.projection_demand_accepted, (try protocol.validateTransportEnvelope(demanded.records.items[0])).kind);
    const frame_envelope = try protocol.validateTransportEnvelope(demanded.records.items[1]);
    try std.testing.expectEqual(MessageKind.snapshot_frame, frame_envelope.kind);
    const frame = try protocol.validateTerminalFrame(std.testing.allocator, frame_envelope.payload);
    try std.testing.expectEqual(@as(u16, 20), frame.columns);
    try std.testing.expectEqual(@as(u16, 4), frame.rows);
    try std.testing.expectEqual(@as(u16, 8), frame.cell_pixel_width);
    try std.testing.expectEqual(@as(u16, 16), frame.cell_pixel_height);
    try std.testing.expectEqual(@as(u64, 1), runtime.projectionCounters().render_state_creations);

    var frame_ack_payload_buffer: [256]u8 = undefined;
    const frame_ack_payload = try std.fmt.bufPrint(
        &frame_ack_payload_buffer,
        "{{\"attachmentId\":\"44444444444444444444444444444444\",\"generation\":\"{d}\",\"revision\":\"{d}\",\"sequence\":\"{d}\"}}",
        .{ frame.generation, frame.revision, frame.sequence },
    );
    const pause_payload =
        \\{"attachmentId":"44444444444444444444444444444444","demand":"paused"}
    ;
    var paused = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        pause_payload,
        [_]u8{0x66} ** 16,
    );
    defer paused.deinit();
    try std.testing.expect(!runtime.projectionResident());
    try std.testing.expect(service.core.findAttachment([_]u8{0x44} ** 16).?.in_flight == null);

    var acknowledged = try testRequest(
        &service,
        &connection,
        MessageKind.frame_applied,
        .control_json,
        frame_ack_payload,
        [_]u8{0x64} ** 16,
    );
    defer acknowledged.deinit();
    try std.testing.expectEqual(MessageKind.frame_applied, (try protocol.validateTransportEnvelope(acknowledged.records.items[0])).kind);

    service.runtimes.getPtr(terminal_id).?.next_projection_at_ns = 0;
    var resumed = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        demand_payload,
        [_]u8{0x67} ** 16,
    );
    defer resumed.deinit();
    try std.testing.expectEqual(@as(usize, 2), resumed.records.items.len);
    const resumed_envelope = try protocol.validateTransportEnvelope(resumed.records.items[1]);
    try std.testing.expectEqual(MessageKind.snapshot_frame, resumed_envelope.kind);
    const resumed_frame = try protocol.validateTerminalFrame(std.testing.allocator, resumed_envelope.payload);
    try std.testing.expectEqual(frame.sequence + 1, resumed_frame.sequence);
    try std.testing.expectEqual(@as(u64, 2), runtime.projectionCounters().render_state_creations);

    var paused_again = try testRequest(
        &service,
        &connection,
        MessageKind.set_projection_demand,
        .control_json,
        pause_payload,
        [_]u8{0x68} ** 16,
    );
    defer paused_again.deinit();
    try std.testing.expect(!runtime.projectionResident());
    const counters_before_pause = runtime.projectionCounters();

    const input_text: []const u8 = if (builtin.os.tag == .windows) "orchbee\r\n" else "orchbee\n";
    const text_payload = try std.testing.allocator.alloc(u8, 20 + input_text.len);
    defer std.testing.allocator.free(text_payload);
    @memset(text_payload[0..16], 0x44);
    std.mem.writeInt(u32, text_payload[16..20], @intCast(input_text.len), .little);
    @memcpy(text_payload[20..], input_text);
    var accepted = try testRequest(
        &service,
        &connection,
        MessageKind.text_input,
        .raw_bytes,
        text_payload,
        [_]u8{0x65} ** 16,
    );
    defer accepted.deinit();
    try std.testing.expectEqual(MessageKind.input_accepted, (try protocol.validateTransportEnvelope(accepted.records.items[0])).kind);

    for (0..400) |_| {
        var events = try service.pollEvents(&connection);
        events.deinit();
        if (service.core.findTerminal(terminal_id).?.phase == .exited) break;
        std.Thread.sleep(5 * std.time.ns_per_ms);
    }
    try std.testing.expectEqual(core_module.TerminalPhase.exited, service.core.findTerminal(terminal_id).?.phase);
    const counters_after_exit = runtime.projectionCounters();
    try std.testing.expectEqual(counters_before_pause.captures, counters_after_exit.captures);
    try std.testing.expectEqual(counters_before_pause.encodes, counters_after_exit.encodes);
}

fn testHandshake(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"{s}\",\"frameBytes\":1024,\"inputBytes\":2048,\"protocol\":{{\"major\":1,\"maximumMinor\":0,\"minimumMinor\":0}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{client_id},
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records.items.len);
    try std.testing.expectEqual(MessageKind.hello_accepted, (try protocol.validateTransportEnvelope(outgoing.records.items[0])).kind);
}

fn testHandshakeV1_1(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"{s}\",\"frameBytes\":1048576,\"inputBytes\":2048,\"protocol\":{{\"major\":1,\"maximumMinor\":1,\"minimumMinor\":1}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{client_id},
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records.items.len);
    const response = try protocol.validateTransportEnvelope(outgoing.records.items[0]);
    try std.testing.expectEqual(MessageKind.hello_accepted, response.kind);
    try std.testing.expectEqual(@as(u16, 1), response.protocol_minor);
}

fn testHandshakeV1_2(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    try testHandshakeVersion(service, connection, client_id, 2);
}

fn testHandshakeV1_3(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    try testHandshakeVersion(service, connection, client_id, 3);
}

fn testHandshakeV1_4(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
) !void {
    try testHandshakeVersion(service, connection, client_id, 4);
}

fn testHandshakeVersion(
    service: *HostService,
    connection: *ConnectionState,
    client_id: []const u8,
    protocol_minor: u16,
) !void {
    const payload = try std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"clientId\":\"{s}\",\"frameBytes\":1048576,\"inputBytes\":2048,\"protocol\":{{\"major\":1,\"maximumMinor\":{d},\"minimumMinor\":{d}}},\"rowWindowBytes\":4096,\"supportedOptionalFeatures\":[]}}",
        .{ client_id, protocol_minor, protocol_minor },
    );
    defer std.testing.allocator.free(payload);
    const record = try protocol.encodeTransportEnvelope(
        std.testing.allocator,
        MessageKind.hello,
        .control_json,
        payload,
        null,
        null,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    var outgoing = try service.handle(connection, record);
    defer outgoing.deinit();
    try std.testing.expectEqual(@as(usize, 1), outgoing.records.items.len);
    const response = try protocol.validateTransportEnvelope(outgoing.records.items[0]);
    try std.testing.expectEqual(MessageKind.hello_accepted, response.kind);
    try std.testing.expectEqual(protocol_minor, response.protocol_minor);
}

fn testCreatePayloadV1_3(
    creation_id: []const u8,
    executable: []const u8,
    arguments: []const []const u8,
) ![]u8 {
    const environment = [_]EnvironmentWire{};
    const features = [_]u16{22};
    const palette = [_]u32{0} ** 256;
    var writer: std.Io.Writer.Allocating = .init(std.testing.allocator);
    defer writer.deinit();
    try std.json.Stringify.value(.{
        .creationId = creation_id,
        .launch = .{
            .arguments = arguments,
            .cellPixelHeight = 16,
            .cellPixelWidth = 8,
            .columns = 80,
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
                .versionResponse = "Orchbee test",
            },
        },
    }, .{}, &writer.writer);
    return writer.toOwnedSlice();
}

fn testAttachPayload(
    attachment_id: core_module.Identifier,
    terminal_id: core_module.Identifier,
) ![]u8 {
    var attachment_buffer: [32]u8 = undefined;
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"attachmentId\":\"{s}\",\"capabilities\":{{\"input\":true,\"read\":false,\"resize\":false}},\"requestResizeLease\":false,\"terminalId\":\"{s}\"}}",
        .{
            identifierText(attachment_id, &attachment_buffer),
            identifierText(terminal_id, &terminal_buffer),
        },
    );
}

fn testRawInputPayload(
    attachment_id: core_module.Identifier,
    input: []const u8,
) ![]u8 {
    const payload = try std.testing.allocator.alloc(u8, attachment_id.len + input.len);
    @memcpy(payload[0..attachment_id.len], &attachment_id);
    @memcpy(payload[attachment_id.len..], input);
    return payload;
}

fn testTerminalTargetPayload(terminal_id: core_module.Identifier) ![]u8 {
    var terminal_buffer: [32]u8 = undefined;
    return std.fmt.allocPrint(
        std.testing.allocator,
        "{{\"terminalId\":\"{s}\"}}",
        .{identifierText(terminal_id, &terminal_buffer)},
    );
}

fn testIdentifier(namespace: u8, index: usize) core_module.Identifier {
    var result = [_]u8{0} ** 16;
    result[0] = namespace;
    std.mem.writeInt(u64, result[8..16], @intCast(index + 1), .little);
    return result;
}

fn testRequest(
    service: *HostService,
    connection: *ConnectionState,
    kind: u16,
    encoding: protocol.PayloadEncoding,
    payload: []const u8,
    correlation_id: core_module.Identifier,
) !Outgoing {
    const record = try protocol.encodeTransportEnvelopeVersion(
        std.testing.allocator,
        connection.protocol_minor,
        kind,
        encoding,
        payload,
        correlation_id,
        service.core.epoch,
        false,
        false,
    );
    defer std.testing.allocator.free(record);
    return service.handle(connection, record);
}
