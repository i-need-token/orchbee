//! The handover record as untrusted bytes (TH-030): a corrupt, truncated, foreign-pin, or
//! unknown-required-section record, or one that waits on a continuation, is refused before
//! anything starts; mutations that keep the checksums never end the Host; and the stored
//! generation-1 sample imports.
const harness = @import("runtime_harness.zig");
const fixture = @import("handover_fixture.zig");
const screen_text = @import("screen_text.zig");
const ScriptedOutput = @import("scripted_output.zig").ScriptedOutput;
const std = harness.std;
const ghostty_vt = harness.ghostty_vt;
const subject = harness.subject;
const handover = fixture.handover;
const envelope = handover.envelope;
const allocator = fixture.allocator;

/// The stored sample's bytes are final: a change to the format is a new generation and a new
/// sample, never a rewrite of this one.
const sample = @embedFile("fixtures/handover-record-1.obhr");

const sample_corpus = "\x1b]2;handover-sample\x1b\\\x1b]7;file://sample-host/work\x1b\\" ++
    // A scroll region homes the cursor, so it comes before the text.
    "\x1b[?1h\x1b[?2004h\x1b[?1002h\x1b[?1006h\x1b[5 q\x1b[3;20r" ++
    "\x1b[1;32msample line\x1b[0m 中文 😀 \x1b]8;id=s;https://example.com/sample\x1b\\link\x1b]8;;\x1b\\\r\n" ++
    "\x07\x1b]52;c;c2FtcGxl\x1b\\\x1b]133;P\x1b\\sample> \x1b]133;B\x1b\\";

/// A record of a terminal fed `sample_corpus`; the caller frees it.
fn sourceRecord(stand_in: *const fixture.StandIn) ![]u8 {
    var output: ScriptedOutput = .{};
    const source = try fixture.createScripted(&output, stand_in, 80, 24);
    defer source.destroy();
    output.commit(sample_corpus);
    return fixture.exportParked(source);
}

/// Imports `bytes` up to the start of its first worker; a refusal must come earlier.
fn expectRefused(stand_in: *const fixture.StandIn, bytes: []const u8, expected: ?anyerror) !void {
    var spawner: fixture.RefusingSpawner = .{};
    const result = subject.testing.importHandoverUsingSpawner(allocator, bytes, stand_in.launch(80, 24), &spawner);
    if (result) |_| return error.TestUnexpectedResult else |err| {
        if (expected) |value| try std.testing.expectEqual(value, err);
        try std.testing.expect(err != error.InjectedSpawnFailure);
    }
    try std.testing.expectEqual(@as(usize, 0), spawner.spawns);
}

/// Whether the import of `bytes` reached its first worker, proving the record was accepted, or was
/// refused before anything started.
fn reachesWorkers(stand_in: *const fixture.StandIn, bytes: []const u8) !bool {
    var spawner: fixture.RefusingSpawner = .{};
    const result = subject.testing.importHandoverUsingSpawner(allocator, bytes, stand_in.launch(80, 24), &spawner);
    if (result) |_| return error.TestUnexpectedResult else |err| {
        const reached = err == error.InjectedSpawnFailure;
        try std.testing.expectEqual(@as(usize, @intFromBool(reached)), spawner.spawns);
        return reached;
    }
}

/// Re-frames `content` under fresh checksums.
fn reframe(content: handover.Content) ![]u8 {
    return handover.encode(allocator, content);
}

fn contentOf(decoded: *const handover.Decoded) handover.Content {
    return .{
        .product_version = decoded.product_version,
        .pin = subject.handover_pin,
        .terminal = decoded.terminal,
        .configuration = decoded.configuration,
        .screen = decoded.screen,
        .engine = decoded.engine,
        .console = decoded.console,
    };
}

test "a corrupt, truncated, foreign-pin, or unknown-required-section record, or one with pending continuation, is refused and creates nothing" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    const record = try sourceRecord(&stand_in);
    defer allocator.free(record);
    try std.testing.expect(try reachesWorkers(&stand_in, record));

    for (0..record.len) |length| try expectRefused(&stand_in, record[0..length], null);
    const corrupt = try allocator.dupe(u8, record);
    defer allocator.free(corrupt);
    for (0..record.len) |index| {
        corrupt[index] ^= 0x01;
        try expectRefused(&stand_in, corrupt, null);
        corrupt[index] ^= 0x01;
    }

    var decoded = try handover.decode(allocator, record, subject.handover_pin);
    defer decoded.deinit();
    {
        var content = contentOf(&decoded);
        content.pin = "ghostty-1.3.2-dev-foreign";
        const foreign = try reframe(content);
        defer allocator.free(foreign);
        try expectRefused(&stand_in, foreign, error.ForeignPin);
    }
    {
        var content = contentOf(&decoded);
        content.engine.scanner_partial_state = "\x1b]7501;state=";
        const scanning = try reframe(content);
        defer allocator.free(scanning);
        try expectRefused(&stand_in, scanning, error.ContinuationPending);
    }
    {
        var content = contentOf(&decoded);
        content.engine.conpty = .{ .clear = true, .size = null, .size_after_clear = null };
        const signalling = try reframe(content);
        defer allocator.free(signalling);
        try expectRefused(&stand_in, signalling, error.SignalPending);
    }
    {
        // Ghostty's snapshot with an unfinished OSC 8 in its continuation record.
        var reader: std.Io.Reader = .fixed(decoded.screen);
        var snapshot = try ghostty_vt.snapshot.decodeExact(allocator, harness.thread_sync.io(), &reader, .{ .max_continuation_bytes = 0 });
        defer snapshot.deinit(allocator);
        var screen: std.Io.Writer.Allocating = .init(allocator);
        defer screen.deinit();
        try ghostty_vt.snapshot.encode(allocator, &screen.writer, &snapshot.terminal.?, .{
            .continuation = .{ .bytes = "\x1b]8;;https://example.com/cut" },
        });
        var content = contentOf(&decoded);
        content.screen = screen.written();
        const continuing = try reframe(content);
        defer allocator.free(continuing);
        try expectRefused(&stand_in, continuing, error.ContinuationPending);
    }
    {
        const framed = try envelope.decode(record, subject.handover_pin);
        var sections: [envelope.max_sections]envelope.Section = undefined;
        @memcpy(sections[0..framed.count], framed.sections[0..framed.count]);
        sections[framed.count] = .{ .tag = @enumFromInt(9), .required = true, .payload = "future" };
        const header: envelope.Header = .{ .product_version = framed.header.product_version, .pin = framed.header.pin };
        const unknown = try envelope.encode(allocator, header, sections[0 .. framed.count + 1]);
        defer allocator.free(unknown);
        try expectRefused(&stand_in, unknown, error.UnknownRequiredSection);
        // A section a later Host may add without requiring it is read past.
        sections[framed.count].required = false;
        const optional = try envelope.encode(allocator, header, sections[0 .. framed.count + 1]);
        defer allocator.free(optional);
        try std.testing.expect(try reachesWorkers(&stand_in, optional));
    }
}

test "mutations of the Host's sections under fresh checksums are refused or restore without ending the Host" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    const record = try sourceRecord(&stand_in);
    defer allocator.free(record);
    const framed = try envelope.decode(record, subject.handover_pin);
    const header: envelope.Header = .{ .product_version = framed.header.product_version, .pin = framed.header.pin };

    var refused: usize = 0;
    var accepted: usize = 0;
    for (0..framed.count) |section_index| {
        // Ghostty's snapshot carries its own checksums and malformed-input evidence.
        if (framed.sections[section_index].tag == .screen) continue;
        const payload = try allocator.dupe(u8, framed.sections[section_index].payload);
        defer allocator.free(payload);
        var sections = framed.sections;
        sections[section_index].payload = payload;
        for (payload) |*byte| {
            for ([_]u8{ 0x01, 0xff }) |mask| {
                byte.* ^= mask;
                defer byte.* ^= mask;
                const mutated = try envelope.encode(allocator, header, sections[0..framed.count]);
                defer allocator.free(mutated);
                if (try reachesWorkers(&stand_in, mutated)) accepted += 1 else refused += 1;
            }
        }
    }
    std.debug.print("handover mutations: {d} refused, {d} accepted\n", .{ refused, accepted });
    try std.testing.expect(refused > 0 and accepted > 0);
}

/// A copy of the snapshot `screen` whose first PAGE header asks for these capacities, under a fresh
/// record checksum; the caller frees it.
fn forgePage(screen: []const u8, columns: u16, rows: u16, grapheme_bytes: u32, string_bytes: u32) ![]u8 {
    const snapshot = ghostty_vt.snapshot;
    const forged = try allocator.dupe(u8, screen);
    errdefer allocator.free(forged);
    var offset: usize = snapshot.envelope.encoded_len;
    while (true) {
        var reader: std.Io.Reader = .fixed(forged[offset..]);
        const header = try snapshot.record.Header.decode(&reader);
        const payload = forged[offset + snapshot.record.Header.len ..][0..header.payload_len];
        try std.testing.expect(header.tag != .finish);
        if (header.tag == .page) {
            std.mem.writeInt(u16, payload[0..2], columns, .little);
            std.mem.writeInt(u16, payload[2..4], rows, .little);
            std.mem.writeInt(u32, payload[12..16], grapheme_bytes, .little);
            std.mem.writeInt(u32, payload[16..20], string_bytes, .little);
            var crc: std.hash.crc.Crc32Iscsi = .init();
            crc.update(forged[offset..][0..6]);
            crc.update(payload);
            std.mem.writeInt(u32, forged[offset + 6 ..][0..4], crc.final(), .little);
            return forged;
        }
        offset += snapshot.record.Header.len + header.payload_len;
    }
}

test "a screen section whose pages would exceed the page bound is refused before Ghostty allocates them" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    const record = try sourceRecord(&stand_in);
    defer allocator.free(record);
    var decoded = try handover.decode(allocator, record, subject.handover_pin);
    defer decoded.deinit();
    const memory = try handover.screen_pages.pageMemory(decoded.screen);
    try std.testing.expect(memory > 0 and memory <= handover.max_page_memory_bytes);

    const framed = try envelope.decode(record, subject.handover_pin);
    const header: envelope.Header = .{ .product_version = framed.header.product_version, .pin = framed.header.pin };
    const screen_index = for (framed.sections[0..framed.count], 0..) |section, index| {
        if (section.tag == .screen) break index;
    } else return error.TestUnexpectedResult;
    // Past Ghostty's 4 GiB page assert, and about 1 GiB, which Ghostty would allocate and zero.
    for ([_][4]u32{
        .{ 0xffff, 0xffff, 0xffff_ffff, 0xffff_ffff },
        .{ 80, 24, 1 << 30, 0 },
    }) |capacities| {
        const forged = try forgePage(decoded.screen, @intCast(capacities[0]), @intCast(capacities[1]), capacities[2], capacities[3]);
        defer allocator.free(forged);
        try std.testing.expectError(error.TooLarge, handover.screen_pages.pageMemory(forged));
        var content = contentOf(&decoded);
        content.screen = forged;
        try std.testing.expectError(error.RecordTooLarge, reframe(content));
        var sections = framed.sections;
        sections[screen_index].payload = forged;
        const forged_record = try envelope.encode(allocator, header, sections[0..framed.count]);
        defer allocator.free(forged_record);
        try expectRefused(&stand_in, forged_record, error.RecordTooLarge);
    }
}

test "the stored generation-1 sample imports" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var output: ScriptedOutput = .{};
    var imported = try fixture.importScripted(sample, &output, &stand_in);
    defer imported.deinit();
    try std.testing.expectEqualStrings("0.1.0-beta.16", imported.facts.product_version);
    try std.testing.expectEqual(@as(u16, 80), imported.facts.terminal.geometry.columns);
    try std.testing.expectEqual(@as(u16, 24), imported.facts.terminal.geometry.rows);
    output.commit("echo after-import\r\n");

    var capture = try screen_text.read(imported.runtime, allocator, 64 * 1024);
    defer capture.deinit();
    try std.testing.expect(std.mem.indexOf(u8, capture.text(), "sample line 中文 😀 link") != null);
    try std.testing.expect(std.mem.indexOf(u8, capture.text(), "sample> echo after-import") != null);
    const state = harness.runtimeState(imported.runtime);
    state.terminal_mutex.lock();
    defer state.terminal_mutex.unlock();
    try std.testing.expectEqualStrings("handover-sample", state.terminal.getTitle().?);
}
