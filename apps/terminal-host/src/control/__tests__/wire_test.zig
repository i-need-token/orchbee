const std = @import("std");
const control_boundary = @import("terminal-host-control");
const core_boundary = @import("terminal-host-core");
const engine_boundary = @import("terminal-host-engine");
const protocol = @import("terminal-protocol");

const encode = control_boundary.encode;
const subject = control_boundary.wire;
const control_decode = control_boundary.decode;
const core_module = core_boundary.state;
const terminal_history = engine_boundary.history;

test "control decoder owns JSON syntax and strict object validation" {
    const valid = try subject.decode(
        subject.TerminalTarget,
        std.testing.allocator,
        "{\"terminalId\":\"11111111111111111111111111111111\"}",
    );
    defer valid.deinit();
    try std.testing.expectEqualStrings(
        "11111111111111111111111111111111",
        valid.value.terminalId,
    );

    try std.testing.expectError(
        error.InvalidControlJson,
        subject.decode(
            subject.TerminalTarget,
            std.testing.allocator,
            "{\"terminalId\":\"11111111111111111111111111111111\",\"extra\":true}",
        ),
    );
    try std.testing.expectError(
        error.InvalidControlJson,
        subject.decode(subject.TerminalTarget, std.testing.allocator, "{"),
    );
    try std.testing.expectError(
        error.InvalidControlJson,
        subject.decode(
            subject.TerminalTarget,
            std.testing.allocator,
            "{\"terminalId\":\"11111111111111111111111111111111\",\"terminalId\":\"22222222222222222222222222222222\"}",
        ),
    );
    try std.testing.expectError(
        error.InvalidControlJson,
        subject.decode(
            subject.TerminalTarget,
            std.testing.allocator,
            "{\"terminalId\":\"11111111111111111111111111111111\",\"\\u0074erminalId\":\"22222222222222222222222222222222\"}",
        ),
    );
    try std.testing.expectError(
        error.InvalidControlJson,
        subject.decode(
            subject.TerminalTarget,
            std.testing.allocator,
            "{\"terminalId\":\"\\ud800\"}",
        ),
    );
}

test "control acknowledgements are encoded from typed values" {
    const attachment_id = core_module.AttachmentId.fromBytes([_]u8{0x11} ** 16);
    var storage: [512]u8 = undefined;

    var output: std.Io.Writer = .fixed(&storage);
    try encode.writeFrameApplied(&output, control_decode.FrameApplied{
        .attachment_id = attachment_id,
        .generation = 2,
        .revision = 3,
        .sequence = 4,
    });
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"generation\":\"2\",\"revision\":\"3\",\"sequence\":\"4\"}",
        output.buffered(),
    );

    output = .fixed(&storage);
    try encode.writeNeedSnapshot(&output, control_decode.NeedSnapshot{
        .attachment_id = attachment_id,
        .reason = .sequence_gap,
        .sequence = 5,
    });
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"reason\":\"sequence-gap\",\"sequence\":\"5\"}",
        output.buffered(),
    );

    output = .fixed(&storage);
    try encode.writeRowsApplied(&output, control_decode.RowsApplied{
        .attachment_id = attachment_id,
        .generation = 6,
        .request_sequence = 7,
        .revision = 8,
    });
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"generation\":\"6\",\"requestSequence\":\"7\",\"revision\":\"8\"}",
        output.buffered(),
    );

    output = .fixed(&storage);
    try encode.writeHistoryDemandAccepted(&output, control_decode.HistoryDemand{
        .attachment_id = attachment_id,
        .demand = terminal_history.HistoryDemand.following_bottom,
    });
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"demand\":\"following-bottom\"}",
        output.buffered(),
    );

    output = .fixed(&storage);
    try encode.writeHistoryObserved(&output, control_decode.HistoryObserved{
        .attachment_id = attachment_id,
        .generation = 9,
        .revision = 10,
    });
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"generation\":\"9\",\"revision\":\"10\"}",
        output.buffered(),
    );

    output = .fixed(&storage);
    try encode.writeSelectionExportChunkApplied(
        &output,
        control_decode.SelectionExportChunkApplied{
            .attachment_id = attachment_id,
            .chunk_sequence = 11,
            .export_sequence = 12,
        },
    );
    try std.testing.expectEqualStrings(
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"chunkSequence\":11,\"exportSequence\":\"12\"}",
        output.buffered(),
    );
}

test "control encoder escapes typed failure text inside its owned wire shape" {
    const encoded = try encode.typedFailure(
        std.testing.allocator,
        "invalid-field",
        "quoted \"message\"\nnext line",
        false,
    );
    defer std.testing.allocator.free(encoded);

    const Failure = struct {
        code: []const u8,
        message: []const u8,
        retryable: bool,
    };
    const parsed = try subject.decode(Failure, std.testing.allocator, encoded);
    defer parsed.deinit();
    try std.testing.expectEqualStrings("invalid-field", parsed.value.code);
    try std.testing.expectEqualStrings("quoted \"message\"\nnext line", parsed.value.message);
    try std.testing.expect(!parsed.value.retryable);

    try std.testing.expectError(
        error.InvalidControlValue,
        encode.typedFailure(
            std.testing.allocator,
            "",
            "message",
            false,
        ),
    );
    try std.testing.expectError(
        error.InvalidControlValue,
        encode.typedFailure(
            std.testing.allocator,
            "code",
            &[_]u8{0xff},
            false,
        ),
    );
}

test "wire presence distinguishes missing fields from explicit null" {
    const parsed = try subject.decode(
        subject.SelectionCommand,
        std.testing.allocator,
        "{\"attachmentId\":\"11111111111111111111111111111111\",\"command\":\"clear\",\"generation\":\"1\",\"point\":null,\"revision\":\"1\"}",
    );
    defer parsed.deinit();
    try std.testing.expectEqual(.null_value, parsed.value.point);
}

test "inventory encoder paginates before the control JSON byte limit" {
    const FailedTerminal = struct {
        columns: u16 = 4096,
        configuration: struct { scrollback_bytes: usize } = .{
            .scrollback_bytes = protocol.Limits.scrollback_bytes,
        },
        configuration_revision: u64 = std.math.maxInt(u64),
        creation_id: core_module.CreationId,
        revision: u64 = std.math.maxInt(u64),
        rows: u16 = 4096,
        terminal_id: core_module.TerminalId,

        pub fn exitResult(_: @This()) ?core_module.ExitResult {
            return null;
        }

        pub fn failedProcess(_: @This()) ?core_module.ProcessObservation {
            return .{ .observed = .{ .unknown = std.math.maxInt(u32) } };
        }

        pub fn failureContainment(_: @This()) ?core_module.FailureContainment {
            return null;
        }

        pub fn effectiveConfiguration(self: @This()) struct { scrollback_bytes: usize } {
            return .{ .scrollback_bytes = self.configuration.scrollback_bytes };
        }

        pub fn configurationRevision(self: @This()) u64 {
            return self.configuration_revision;
        }

        pub fn observedGeometry(self: @This()) struct { columns: u16, rows: u16 } {
            return .{ .columns = self.columns, .rows = self.rows };
        }

        pub fn observedRevision(self: @This()) u64 {
            return self.revision;
        }

        pub fn phase(_: @This()) core_module.TerminalPhase {
            return .failed;
        }

        pub fn runtimeFailure(_: @This()) ?core_module.RuntimeFailure {
            return .synchronize_worker;
        }

        pub fn terminateRequested(_: @This()) bool {
            return true;
        }
    };
    var terminals: [protocol.Limits.inventory_page_terminals]FailedTerminal = undefined;
    for (&terminals, 0..) |*terminal, index| {
        const byte: u8 = @intCast(index + 1);
        terminal.* = .{
            .creation_id = core_module.CreationId.fromBytes([_]u8{byte} ** 16),
            .terminal_id = core_module.TerminalId.fromBytes([_]u8{byte} ** 16),
        };
    }
    const payload = try encode.terminalList(
        std.testing.allocator,
        core_module.HostEpoch.fromBytes([_]u8{0xaa} ** 16),
        std.math.maxInt(u64),
        &terminals,
        0,
        terminals.len,
    );
    defer std.testing.allocator.free(payload);
    try std.testing.expect(payload.len <= protocol.Limits.control_json_bytes);

    const Page = struct {
        inventoryGeneration: []const u8,
        nextCursor: ?subject.InventoryCursor,
        terminals: []std.json.Value,
    };
    const parsed = try subject.decode(Page, std.testing.allocator, payload);
    defer parsed.deinit();
    try std.testing.expect(parsed.value.terminals.len > 0);
    try std.testing.expect(parsed.value.terminals.len < terminals.len);
    try std.testing.expectEqual(
        parsed.value.terminals.len,
        try std.fmt.parseInt(usize, parsed.value.nextCursor.?.offset, 10),
    );
}

test "fixed control writer matches allocating encoder and preserves escaping" {
    const code = "invalid-\"field";
    const message = "quoted \"message\"\nnext line";
    const allocated = try encode.typedFailure(
        std.testing.allocator,
        code,
        message,
        true,
    );
    defer std.testing.allocator.free(allocated);

    var storage: [512]u8 = undefined;
    var writer: std.Io.Writer = .fixed(&storage);
    try encode.writeTypedFailure(&writer, code, message, true);
    try std.testing.expectEqualSlices(u8, allocated, writer.buffered());
}
