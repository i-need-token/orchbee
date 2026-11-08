const std = @import("std");
const builtin = @import("builtin");
const platform = @import("terminal-host-platform");
const harness = @import("runtime_aggregate_harness.zig");
const subject = harness.subject;
const launch = harness.terminal_launch;
const core = harness.core_module;
const allocator = std.testing.allocator;
const io = std.testing.io;
const geometry = @import("terminal-host-model").geometry.Geometry{
    .columns = 80,
    .rows = 24,
    .cell_pixel_width = 0,
    .cell_pixel_height = 0,
};

const Fixture = struct {
    aggregate: subject.HostRuntimeAggregate,
    source: launch.ScrollbackSource,
    /// The folder the Worker would pass for editor documents.
    documents: std.testing.TmpDir,
    document_directory: [:0]u8,
    live: bool = true,

    fn init() !Fixture {
        var documents = std.testing.tmpDir(.{});
        errdefer documents.cleanup();
        const document_directory = try documents.parent_dir.realPathFileAlloc(io, &documents.sub_path, allocator);
        errdefer allocator.free(document_directory);
        var aggregate = subject.HostRuntimeAggregate.init(allocator);
        aggregate.editor_document_directory = document_directory;
        errdefer aggregate.deinitChecked(false) catch @panic("fixture initialization cleanup failed");
        _ = try aggregate.connect(1, harness.testId(core.ClientId, 0x62));
        var source_launch = try harness.validatedLongRunningLaunch();
        defer source_launch.deinit();
        var creation = try aggregate.prepareCreateTerminal(1, harness.testId(core.CreationId, 0x63), source_launch.validated);
        defer creation.deinit();
        const identity = (try aggregate.commitPreparedTerminal(&creation)).identity();
        const runtime = aggregate.runtimeFor(identity.terminal_id).?;
        _ = try aggregate.attach(1, harness.testId(core.AttachmentId, 0x71), identity.terminal_id, .{ .read = false, .input = true, .resize = false }, false);
        try std.testing.expect(harness.commitReaderBatch(runtime, "first line\r\n保留 😀 e\xcc\x81\r\nprompt>") == .committed);
        // Freeze a legitimate retained readable failure after containing and
        // joining its real Job. Console startup output can otherwise race a
        // deliberately stale revision test; live capture is covered in engine.
        harness.recordReaderFailure(runtime);
        aggregate.synchronizeTerminal(identity.terminal_id);
        try std.testing.expectEqual(core.TerminalPhase.failed, aggregate.coreRead().findTerminal(identity.terminal_id).?.phase());
        const attachment_id = harness.testId(core.AttachmentId, 0x64);
        _ = try aggregate.attach(1, attachment_id, identity.terminal_id, .{ .read = true, .input = false, .resize = false }, false);
        const observed = runtime.geometry();
        return .{
            .aggregate = aggregate,
            .source = .{
                .attachment_id = attachment_id,
                .terminal_id = identity.terminal_id,
                .generation = observed.generation,
                .revision = observed.revision,
            },
            .documents = documents,
            .document_directory = document_directory,
        };
    }

    fn deinit(self: *Fixture) void {
        if (self.live) self.aggregate.deinitChecked(false) catch @panic("fixture aggregate cleanup failed");
        allocator.free(self.document_directory);
        self.documents.cleanup();
    }

    fn prepareEditor(self: *Fixture, identity_byte: u8) !subject.CreatePreparation {
        var editor = harness.validMissingExecutableLaunch();
        editor.geometry = null;
        editor.arguments = &.{ "--literal", "space & 中文" };
        const validated = try launch.inspect(allocator, editor);
        return self.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, identity_byte), validated, self.source);
    }

    fn failedStart(self: *Fixture, terminal_id: core.TerminalId) !void {
        var preparation = try self.aggregate.prepareStartTerminal(1, terminal_id, geometry);
        try std.testing.expectError(error.RuntimeSpawnFailed, self.aggregate.commitPreparedTerminalStart(&preparation));
    }
};

fn expectAbsent(path: []const u8) !void {
    if (std.Io.Dir.cwd().openFile(io, path, .{})) |file| {
        file.close(io);
        return error.UnexpectedEditorDocument;
    } else |err| try std.testing.expectEqual(error.FileNotFound, err);
}

fn expectContents(path: []const u8, text: []const u8) !void {
    const contents = try std.Io.Dir.cwd().readFileAlloc(io, path, allocator, .limited(4096));
    defer allocator.free(contents);
    try std.testing.expectEqualStrings(text, contents);
}

fn expectCreationRefused(expected: anyerror, result: subject.CreatePrepareError!subject.CreatePreparation) !void {
    if (result) |accepted| {
        var preparation = accepted;
        defer preparation.deinit();
        return error.ExpectedEditorCreationRefusal;
    } else |err| try std.testing.expectEqual(expected, err);
}

fn preparedPath(preparation: *const subject.CreatePreparation) ![]u8 {
    const arguments = preparation.pending.runtime_preparation.arguments;
    try std.testing.expectEqual(@as(usize, 3), arguments.len);
    try std.testing.expectEqualStrings("--literal", arguments[0]);
    try std.testing.expectEqualStrings("space & 中文", arguments[1]);
    try std.testing.expect(std.fs.path.isAbsolute(arguments[2]));
    return allocator.dupe(u8, arguments[2]);
}

test "editor deferred lifecycle commits ownership before writing and retains immutable text on failed Start" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try Fixture.init();
    defer fixture.deinit();
    var expected = try fixture.aggregate.runtimeFor(fixture.source.terminal_id).?.captureScrollbackText(allocator, 4096);
    defer expected.deinit();
    var preparation = try fixture.prepareEditor(0x65);
    defer preparation.deinit();
    const path = try preparedPath(&preparation);
    defer allocator.free(path);
    try expectAbsent(path);
    try std.testing.expectEqual(@as(usize, 1), fixture.aggregate.terminalCount());
    try std.testing.expect(!preparation.startsImmediately());
    const editor = (try fixture.aggregate.commitPreparedTerminal(&preparation)).identity();
    try expectAbsent(path);
    try std.testing.expectEqual(@as(usize, 2), fixture.aggregate.terminalCount());
    try std.testing.expectEqual(@as(usize, 1), fixture.aggregate.runtimeCount());
    try harness.publishPendingOutcome(&fixture.aggregate, fixture.source.terminal_id);
    try std.testing.expectEqual(core.TransitionOutcome.applied, (try fixture.aggregate.disposeTerminal(1, fixture.source.terminal_id)).transition);
    try fixture.failedStart(editor.terminal_id);
    try expectContents(path, expected.text());
    try std.testing.expectEqual(core.TerminalPhase.awaiting_start, fixture.aggregate.coreRead().findTerminal(editor.terminal_id).?.phase());
    var replay = try fixture.prepareEditor(0x65);
    defer replay.deinit();
    try std.testing.expect(!replay.requiresEffect());
    try std.testing.expect(replay.identity().terminal_id.eql(editor.terminal_id));
    var changed_source = fixture.source;
    changed_source.revision += 1;
    var editor_launch = harness.validMissingExecutableLaunch();
    editor_launch.geometry = null;
    editor_launch.arguments = &.{ "--literal", "space & 中文" };
    const validated = try launch.inspect(allocator, editor_launch);
    try expectCreationRefused(error.CreationConflict, fixture.aggregate.prepareCreateTerminalWithScrollback(1, editor.creation_id, validated, changed_source));
    try expectCreationRefused(error.CreationConflict, fixture.aggregate.prepareCreateTerminal(1, editor.creation_id, validated));
    try fixture.failedStart(editor.terminal_id);
    try expectContents(path, expected.text());
    const disposed = try fixture.aggregate.disposeTerminal(1, editor.terminal_id);
    try std.testing.expectEqual(core.TransitionOutcome.applied, disposed.transition);
    try expectAbsent(path);
}

test "editor deferred lifecycle fences source authority revision generation and immediate launch before ownership" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try Fixture.init();
    defer fixture.deinit();
    var editor = harness.validMissingExecutableLaunch();
    const immediate = try launch.inspect(allocator, editor);
    try expectCreationRefused(error.InvalidEditorLaunch, fixture.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, 0x65), immediate, fixture.source));
    editor.geometry = null;
    const validated = try launch.inspect(allocator, editor);
    var changed = fixture.source;
    changed.generation += 1;
    try expectCreationRefused(error.StaleEditorSource, fixture.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, 0x65), validated, changed));
    changed = fixture.source;
    changed.revision += 1;
    try expectCreationRefused(error.StaleEditorSource, fixture.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, 0x65), validated, changed));
    changed = fixture.source;
    changed.generation = 0;
    try expectCreationRefused(error.InvalidEditorLaunch, fixture.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, 0x65), validated, changed));
    changed = fixture.source;
    changed.terminal_id = harness.testId(core.TerminalId, 0x70);
    try expectCreationRefused(error.EditorSourceUnavailable, fixture.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, 0x65), validated, changed));
    changed = fixture.source;
    changed.attachment_id = harness.testId(core.AttachmentId, 0x71);
    try expectCreationRefused(error.EditorSourceUnavailable, fixture.aggregate.prepareCreateTerminalWithScrollback(1, harness.testId(core.CreationId, 0x65), validated, changed));
    try expectCreationRefused(error.MissingConnection, fixture.aggregate.prepareCreateTerminalWithScrollback(2, harness.testId(core.CreationId, 0x65), validated, fixture.source));
    try std.testing.expectEqual(@as(usize, 1), fixture.aggregate.terminalCount());
    try std.testing.expectEqual(@as(usize, 1), fixture.aggregate.runtimeCount());
}

extern "kernel32" fn CreateFileW(
    name: [*:0]const u16,
    access: u32,
    sharing: u32,
    security: ?*anyopaque,
    disposition: u32,
    attributes: u32,
    template: ?std.os.windows.HANDLE,
) callconv(.winapi) std.os.windows.HANDLE;

fn denyDeletion(path: []const u8) !std.os.windows.HANDLE {
    const wide = try std.unicode.utf8ToUtf16LeAllocZ(allocator, path);
    defer allocator.free(wide);
    const handle = CreateFileW(wide, 0x80000000, 3, null, 3, 0x80, null);
    if (handle == std.os.windows.INVALID_HANDLE_VALUE) return error.BlockerOpenFailed;
    return handle;
}

test "editor deferred lifecycle schedules blocked Dispose cleanup and permanently denies later Start" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try Fixture.init();
    defer fixture.deinit();
    var clock: harness.LogicalCleanupClock = .{};
    subject.testing.setCleanupClockForTesting(&fixture.aggregate, harness.readLogicalCleanupNs, &clock);
    var preparation = try fixture.prepareEditor(0x65);
    defer preparation.deinit();
    const path = try preparedPath(&preparation);
    defer allocator.free(path);
    const editor = (try fixture.aggregate.commitPreparedTerminal(&preparation)).identity();
    try fixture.failedStart(editor.terminal_id);
    {
        const blocker = try denyDeletion(path);
        defer _ = std.os.windows.CloseHandle(blocker);
        const disposed = try fixture.aggregate.disposeTerminal(1, editor.terminal_id);
        try std.testing.expectEqual(core.RuntimeCleanupFailure.release_editor_document, disposed.cleanup_blocked);
        const retained = fixture.aggregate.coreRead().findTerminal(editor.terminal_id).?;
        try std.testing.expect(retained.pendingOutcome() == null);
        try std.testing.expectEqual(core.TerminalPhase.awaiting_start, retained.phase());
        try std.testing.expectEqual(@as(i128, std.time.ns_per_s), retained.retainedCleanupRetryAt().?);
        try std.testing.expect(fixture.aggregate.pendingStartCleanup(editor.terminal_id));
        try std.testing.expectError(error.InvalidPhase, fixture.aggregate.prepareStartTerminal(1, editor.terminal_id, geometry));
        fixture.aggregate.synchronizeRuntimes();
        try std.testing.expect(retained.retainedCleanupPending());
    }
    clock.now_ns = std.time.ns_per_s;
    fixture.aggregate.synchronizeRuntimes();
    try std.testing.expect(!fixture.aggregate.pendingStartCleanup(editor.terminal_id));
    try std.testing.expectEqual(@as(?core.RuntimeCleanupFailure, null), fixture.aggregate.coreRead().findTerminal(editor.terminal_id).?.retainedCleanupFailure());
    try std.testing.expectError(error.InvalidPhase, fixture.aggregate.prepareStartTerminal(1, editor.terminal_id, geometry));
    try expectAbsent(path);
    try std.testing.expectEqual(core.TransitionOutcome.applied, (try fixture.aggregate.disposeTerminal(1, editor.terminal_id)).transition);
}

test "editor deferred lifecycle fatal teardown exhaustion retains the document without a fictitious child outcome" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try Fixture.init();
    defer fixture.deinit();
    var clock: harness.LogicalCleanupClock = .{};
    subject.testing.setCleanupClockForTesting(&fixture.aggregate, harness.readLogicalCleanupNs, &clock);
    var preparation = try fixture.prepareEditor(0x65);
    defer preparation.deinit();
    const path = try preparedPath(&preparation);
    defer allocator.free(path);
    const editor = (try fixture.aggregate.commitPreparedTerminal(&preparation)).identity();
    try fixture.failedStart(editor.terminal_id);
    {
        const blocker = try denyDeletion(path);
        defer _ = std.os.windows.CloseHandle(blocker);
        try std.testing.expectError(error.EditorDocumentReleaseFailed, fixture.aggregate.deinitChecked(false));
        try std.testing.expect(fixture.aggregate.retainedCleanupPending());
        try std.testing.expect(fixture.aggregate.retainedCleanupStatus().active);
        try std.testing.expectEqual(subject.testing.RetainedCleanupResultType.retained_failed, fixture.aggregate.runRetainedCleanupWindow());
        const retained = fixture.aggregate.coreRead().findTerminal(editor.terminal_id).?;
        try std.testing.expect(retained.pendingOutcome() == null);
        try std.testing.expectEqual(core.TerminalPhase.awaiting_start, retained.phase());
        try std.testing.expectEqual(core.RuntimeCleanupFailure.release_editor_document, retained.retainedCleanup().exhausted);
        try std.testing.expect(fixture.aggregate.pendingStartCleanup(editor.terminal_id));
    }
    try fixture.aggregate.finishRetainedCleanup();
    fixture.live = false;
    try expectAbsent(path);
}

test "editor deferred lifecycle writes beside a foreign document of the same name and never removes it" {
    if (builtin.os.tag != .windows) return error.SkipZigTest;
    var fixture = try Fixture.init();
    defer fixture.deinit();
    var expected = try fixture.aggregate.runtimeFor(fixture.source.terminal_id).?.captureScrollbackText(allocator, 4096);
    defer expected.deinit();
    var preparation = try fixture.prepareEditor(0x65);
    defer preparation.deinit();
    const path = try preparedPath(&preparation);
    defer allocator.free(path);
    try std.testing.expect(std.mem.startsWith(u8, path, fixture.document_directory));
    try std.Io.Dir.cwd().writeFile(io, .{ .sub_path = path, .data = "foreign fixture owner" });
    const next = try std.mem.concat(allocator, u8, &.{ path[0 .. path.len - ".txt".len], "-2.txt" });
    defer allocator.free(next);
    const editor = (try fixture.aggregate.commitPreparedTerminal(&preparation)).identity();
    try fixture.failedStart(editor.terminal_id);
    try expectContents(next, expected.text());
    try expectContents(path, "foreign fixture owner");
    try std.testing.expectEqual(core.TransitionOutcome.applied, (try fixture.aggregate.disposeTerminal(1, editor.terminal_id)).transition);
    try expectAbsent(next);
    try expectContents(path, "foreign fixture owner");
}

test "editor deferred lifecycle digest distinguishes every source field and ordinary launch identity" {
    const ordinary = [_]u8{0x21} ** 32;
    const source: launch.ScrollbackSource = .{
        .attachment_id = harness.testId(core.AttachmentId, 0x62),
        .terminal_id = harness.testId(core.TerminalId, 0x63),
        .generation = 7,
        .revision = 0,
    };
    const original = launch.editorLaunchDigest(ordinary, source);
    try std.testing.expect(!std.mem.eql(u8, &ordinary, &original));
    var changed = source;
    changed.attachment_id = harness.testId(core.AttachmentId, 0x64);
    try std.testing.expect(!std.mem.eql(u8, &original, &launch.editorLaunchDigest(ordinary, changed)));
    changed = source;
    changed.terminal_id = harness.testId(core.TerminalId, 0x64);
    try std.testing.expect(!std.mem.eql(u8, &original, &launch.editorLaunchDigest(ordinary, changed)));
    changed = source;
    changed.generation = std.math.maxInt(u64);
    try std.testing.expect(!std.mem.eql(u8, &original, &launch.editorLaunchDigest(ordinary, changed)));
    changed = source;
    changed.revision = std.math.maxInt(u64);
    try std.testing.expect(!std.mem.eql(u8, &original, &launch.editorLaunchDigest(ordinary, changed)));
}

test "editor deferred lifecycle private Create dispatch refuses absent source authority before identity or file effects" {
    const transport = @import("service_harness.zig");
    var service = transport.HostService.init(allocator);
    defer service.deinit();
    var connection: transport.ConnectionState = .awaiting_hello;
    defer service.disconnect(&connection);
    try transport.testHandshake(&service, &connection, "22222222222222222222222222222222");
    const ordinary = try transport.testCreatePayload("33333333333333333333333333333333", "editor.exe", &.{});
    defer allocator.free(ordinary);
    var parsed = try std.json.parseFromSlice(std.json.Value, allocator, ordinary, .{});
    defer parsed.deinit();
    const arena = parsed.arena.allocator();
    const launch_object = &parsed.value.object.getPtr("launch").?.object;
    for ([_][]const u8{ "columns", "rows", "cellPixelWidth", "cellPixelHeight" }) |key| {
        try launch_object.put(arena, key, .null);
    }
    var source = try std.json.parseFromSlice(std.json.Value, allocator, "{\"attachmentId\":\"66666666666666666666666666666666\",\"terminalId\":\"44444444444444444444444444444444\",\"generation\":\"1\",\"revision\":\"0\"}", .{});
    defer source.deinit();
    try parsed.value.object.put(arena, "captureSource", source.value);
    const payload = try std.json.Stringify.valueAlloc(allocator, parsed.value, .{});
    defer allocator.free(payload);
    var response = try transport.testRequest(&service, &connection, transport.MessageKind.create_terminal, .control_json, payload, [_]u8{0x67} ** 16);
    defer response.deinit();
    const expected_code = if (builtin.os.tag == .macos) "platform-unsupported" else "missing-resource";
    try transport.expectSingleFailureCode(&response, expected_code);
    try std.testing.expectEqual(@as(usize, 0), transport.serviceCore(&service).terminalCount());
}
