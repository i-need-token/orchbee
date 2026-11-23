//! Exact export and import of one terminal within one Host (TH-030): prototype 2's corpus and a
//! busy shell restore identical state, and a corpus cut at every byte exports only where every
//! parser is at ground and, imported and fed the rest, ends identical to its source. A restored
//! terminal's screen gate refuses every key decided before the move.
const harness = @import("runtime_harness.zig");
const fixture = @import("handover_fixture.zig");
const screen_text = @import("screen_text.zig");
const ScriptedOutput = @import("scripted_output.zig").ScriptedOutput;
const std = harness.std;
const builtin = harness.target_builtin;
const monotonic = harness.platform_boundary.monotonic;
const pty_process = harness.pty_process;
const subject = harness.subject;
const TerminalRuntime = harness.TerminalRuntime;
const ReaderParkWait = subject.ReaderParkWait;
const allocator = fixture.allocator;

const capture_limit_bytes = 512 * 1024;
const output_timeout_ns = 60 * std.time.ns_per_s;
const screen_gate_quiet_ns = harness.protocol.Limits.screen_gate_quiet_milliseconds * std.time.ns_per_ms;

/// Prototype 2's corpus (proposals/platform/host-handover.md, "Prototype 2 result"), with the
/// sequences the Host's own scanners and effects keep beside Ghostty's state.
const prototype_corpus = primary_history ++ primary_state ++ alternate_screen;

const primary_history = history: {
    @setEvalBranchQuota(500_000);
    var bytes: []const u8 = "\x1b]4;1;rgb:12/34/56\x1b\\";
    for (0..60) |line| {
        bytes = bytes ++ std.fmt.comptimePrint(
            "\x1b[1;3{d}m{d:0>3}\x1b[0m \x1b[38;5;{d}mpalette\x1b[0m \x1b[38;2;{d};80;160;48;2;10;20;{d}mrgb\x1b[0m " ++
                "中文字符 😀 👍🏽 e\u{301} " ++
                "\x1b]8;id=doc-{d};https://example.com/doc/{d}\x1b\\explicit\x1b]8;;\x1b\\ " ++
                "\x1b]8;;https://example.com/implicit/{d}\x07implicit\x1b]8;;\x07\r\n",
            .{ line % 8, line, 16 + line, line * 4, line, line, line, line },
        );
        if (line % 10 == 0) bytes = bytes ++ ("w" ** 150) ++ "\r\n";
    }
    break :history bytes;
};

const command_id = "0123456789abcdef0123456789abcdef";
const prompt = "\x1b]133;P\x1b\\handover> \x1b]133;B\x1b\\";

const primary_state = "\x1b]2;handover-title\x1b\\" ++
    "\x1b]7;file://handover-host/work/dir\x1b\\" ++
    "\x1b[?1h\x1b[?2004h\x1b[?1002h\x1b[?1006h\x1b[?1004h\x1b[>4;2m\x1b[5 q" ++
    "\x1b[10;20H\x1b7\x1b[5;35r" ++
    "\x07\x1b]9;4;1;42\x1b\\\x1b]52;c;aGFuZG92ZXI=\x1b\\" ++
    "\x1b_orchbee.agent.v1;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjoxfQ\x1b\\" ++
    prompt ++ "\x1b]133;C\x1b\\\x1b_orchbee.command.v1;start;" ++ command_id ++ "\x1b\\running\r\n" ++
    "\x1b_orchbee.command.v1;finish;" ++ command_id ++ ";0\x1b\\\x1b]133;D;0\x1b\\" ++ prompt ++
    // A prompt ends the program's reported status, so the report follows the last one.
    "\x1b]7501;state=working\x1b\\";

const alternate_screen = alternate: {
    @setEvalBranchQuota(100_000);
    var bytes: []const u8 = "\x1b[?1049h";
    for (1..41) |row| {
        bytes = bytes ++ std.fmt.comptimePrint("\x1b[{d};1H\x1b[3{d}malternate row {d:0>2} ", .{ row, row % 8, row }) ++ ("." ** 90);
    }
    break :alternate bytes ++ "\x1b[0m\x1b[3;30r\x1b[>5u\x1b[4 q\x1b[7;9H\x1b7" ++
        "\x1b]8;id=alt;https://example.com/alt\x1b\\active link";
};

const Segment = struct {
    bytes: []const u8,
    /// A cut inside leaves Ghostty's parser or one of the Host's scanners inside a sequence.
    sequence: bool,
};

/// Prototype 2's corpus in miniature, one of each kind of sequence a parser keeps across reads,
/// so the cut at every byte stays affordable in the debug build.
const cut_segments = [_]Segment{
    .{ .bytes = "a", .sequence = false },
    .{ .bytes = "\x1b[1;31m", .sequence = true },
    .{ .bytes = "b", .sequence = false },
    .{ .bytes = "\x1b[38;2;1;2;3m", .sequence = true },
    .{ .bytes = "中", .sequence = true },
    .{ .bytes = "😀", .sequence = true },
    .{ .bytes = "e", .sequence = false },
    .{ .bytes = "\u{301}", .sequence = true },
    .{ .bytes = "\x1b[0m", .sequence = true },
    .{ .bytes = "\x1b]8;id=x;https://e.x/\x1b\\", .sequence = true },
    .{ .bytes = "L", .sequence = false },
    .{ .bytes = "\x1b]8;;\x1b\\", .sequence = true },
    .{ .bytes = "\x1b]8;;https://i.x/\x07", .sequence = true },
    .{ .bytes = "I", .sequence = false },
    .{ .bytes = "\x1b]8;;\x07", .sequence = true },
    .{ .bytes = "\x1b]2;cut-title\x07", .sequence = true },
    .{ .bytes = "\x1b]7;file://h/w\x1b\\", .sequence = true },
    .{ .bytes = "\x1b]7501;state=idle\x1b\\", .sequence = true },
    .{ .bytes = "\x1b_orchbee.agent.v1;eyJraW5kIjoiY29ycmVsYXRpb24iLCJ2ZXJzaW9uIjoxfQ\x1b\\", .sequence = true },
    .{ .bytes = "\x1b_orchbee.command.v1;ready\x1b\\", .sequence = true },
    .{ .bytes = "\x1bPzz\x1b\\", .sequence = true },
    .{ .bytes = "\x1b[?1002h", .sequence = true },
    .{ .bytes = "\x1b[>4;2m", .sequence = true },
    .{ .bytes = "\x1b[5 q", .sequence = true },
    .{ .bytes = "\x1b7", .sequence = true },
    .{ .bytes = "\x07", .sequence = false },
    .{ .bytes = "\x1b]133;P\x1b\\", .sequence = true },
    .{ .bytes = "$ ", .sequence = false },
    .{ .bytes = "\x1b]133;B\x1b\\", .sequence = true },
    .{ .bytes = "\x1b[?1049h", .sequence = true },
    .{ .bytes = "alt", .sequence = false },
    .{ .bytes = "\x1b[>1u", .sequence = true },
    .{ .bytes = "\x1b[?1049l", .sequence = true },
    .{ .bytes = "\x1b8", .sequence = true },
    .{ .bytes = "g\r\n", .sequence = false },
};

const cut_corpus = blk: {
    var bytes: []const u8 = "";
    for (cut_segments) |segment| bytes = bytes ++ segment.bytes;
    break :blk bytes;
};

/// Whether the cut corpus cut at `offset` leaves every parser and scanner at ground.
fn groundAt(offset: usize) bool {
    var start: usize = 0;
    for (cut_segments) |segment| {
        const end = start + segment.bytes.len;
        if (offset == start or offset == end) return true;
        if (offset < end) return !segment.sequence;
        start = end;
    }
    unreachable;
}

/// Gives the terminal an attachment that holds focus, a key, and a button, then waits until the
/// writer has written what they sent.
fn holdInput(runtime: *TerminalRuntime) !void {
    const attachment_id = subject.AttachmentId.fromBytes([_]u8{0x51} ** 16);
    var attachment = try runtime.prepareAttachment(attachment_id, .{ .input = true, .read = false, .resize = false });
    attachment.commit();
    const held = [_]harness.protocol.SemanticInput{
        .{ .focus = .{ .focused = true } },
        .{ .key = .{
            .action = .press,
            .composing = false,
            .consumed_modifiers = .{},
            .key = "KeyA",
            .modifiers = .{},
            .text = "a",
            .unshifted_codepoint = 'a',
        } },
        .{ .mouse = .{ .action = .{ .press = .left }, .modifiers = .{}, .pressed_buttons = .{ .left = true }, .x = 5.5, .y = 3.25 } },
    };
    for (held) |input| {
        var prepared = try runtime.prepareAttachmentSemanticInput(attachment_id, input, std.math.maxInt(usize));
        defer prepared.deinit();
        _ = try prepared.commit();
    }
    var timer = try monotonic.Timer.start();
    while (harness.inputQueueSnapshot(runtime).entry_count != 0) {
        if (timer.read() >= output_timeout_ns) return error.InputNotWritten;
        monotonic.sleep(5 * std.time.ns_per_ms);
    }
}

test "export then import restores prototype 2's corpus exactly in every section" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var source_output: ScriptedOutput = .{};
    const source = try fixture.createScripted(&source_output, &stand_in, 120, 40);
    defer source.destroy();
    source_output.commit(prototype_corpus);
    try holdInput(source);

    const record = try fixture.exportParked(source);
    defer allocator.free(record);
    var imported_output: ScriptedOutput = .{};
    var imported = try fixture.importScripted(record, &imported_output, &stand_in);
    defer imported.deinit();

    try fixture.expectSameTerminal(source, imported.runtime);
    try fixture.expectDurationsCarried(record, imported.runtime);
    try fixture.expectSameSections(source, imported.runtime);
    try std.testing.expectEqual(@as(usize, 1), imported.facts.held_attachments.len);
    var decoded = try fixture.handover.decode(allocator, record, subject.handover_pin);
    defer decoded.deinit();
    // Every section is present, and nothing in the record waits for a continuation.
    const engine = decoded.engine;
    try std.testing.expect(engine.prompt_gate.anchor != null);
    try std.testing.expect(engine.command_observation.committed.command != null);
    try std.testing.expect(engine.clipboard_pending != null);
    try std.testing.expect(engine.application_signals.len == 1);
    try std.testing.expect(engine.progress.state == .set and engine.program_status != null and engine.command_completion != null);
    try std.testing.expect(engine.bell_sequence == 1 and engine.held_input.len == 1);
    try std.testing.expectEqual(@as(usize, 0), engine.scanner_partial_state.len);
    try std.testing.expect(engine.conpty.empty());

    // Both go on identically.
    const more = "\x1b]8;;\x1b\\ closed\r\nafter import\r\n\x1b[?1049l\x1b]133;C\x1b\\back on the primary screen\r\n";
    source_output.commit(more);
    imported_output.commit(more);
    try fixture.expectSameTerminal(source, imported.runtime);
    try fixture.expectSameSections(source, imported.runtime);
}

/// Images on both screens: one with an internal and an external placement that scroll into
/// history, one with no placement, one known only by its number, and one on the alternate screen.
const image_corpus = "\x1b_Ga=T,t=d,f=24,i=7,s=1,v=2,q=2;AQIDBAUG\x1b\\" ++
    "\x1b_Ga=p,i=7,p=3,c=4,r=2,z=-5,q=2\x1b\\" ++
    "\x1b_Ga=t,t=d,f=32,i=9,s=1,v=1,q=2;AQIDBA==\x1b\\" ++
    "\x1b_Ga=T,t=d,f=24,I=5,s=1,v=1,q=2;/wAA\x1b\\" ++
    ("history\r\n" ** 30) ++
    "\x1b[?1049h\x1b_Ga=T,t=d,f=24,i=11,s=1,v=1,q=2;AP8A\x1b\\";

test "export then import restores each screen's Kitty images and placements, and a virtual placement stays" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var source_output: ScriptedOutput = .{};
    const source = try fixture.createScripted(&source_output, &stand_in, 80, 24);
    defer source.destroy();
    source_output.commit(image_corpus);

    const record = try fixture.exportParked(source);
    defer allocator.free(record);
    var decoded = try fixture.handover.decode(allocator, record, subject.handover_pin);
    defer decoded.deinit();
    try std.testing.expectEqual(@as(usize, 2), decoded.images.len);
    try std.testing.expectEqual(@as(usize, 3), decoded.images[0].images.len);
    try std.testing.expectEqual(@as(usize, 3), decoded.images[0].placements.len);
    try std.testing.expectEqual(@as(usize, 1), decoded.images[1].placements.len);

    var imported_output: ScriptedOutput = .{};
    var imported = try fixture.importScripted(record, &imported_output, &stand_in);
    defer imported.deinit();
    try fixture.expectSameTerminal(source, imported.runtime);
    try fixture.expectSameSections(source, imported.runtime);

    // Both go on identically: an image without an id and a placement without one take the same
    // next ids.
    const more = "\x1b_Ga=T,t=d,f=24,s=1,v=1,q=2;AAD/\x1b\\\x1b[?1049l\x1b_Ga=p,i=7,q=2\x1b\\";
    source_output.commit(more);
    imported_output.commit(more);
    try fixture.expectSameTerminal(source, imported.runtime);
    try fixture.expectSameSections(source, imported.runtime);

    // Unicode placeholders name a virtual placement, which the section does not carry.
    source_output.commit("\x1b_Ga=p,U=1,i=9,c=1,r=1,q=2\x1b\\");
    try std.testing.expectError(error.HoldsImage, fixture.exportParked(source));
}

test "export then import restores a busy shell exactly" {
    const flood = "busy-";
    // A slow reader of a flood gets the PTY's whole buffer each read, 4,095 bytes on Linux. Lines
    // of one length that divides it, such as the 21 bytes of a five-digit line, would end every
    // read at one place in a line, inside a sequence for seconds; the padding varies the length.
    const posix_command = "i=1; pad=; while :; do printf '\\033[3%dm" ++ flood ++ "%d%s\\033[0m\\n' $((i%8)) $i \"$pad\"; i=$((i+1)); pad=${pad}x; [ ${#pad} -gt 9 ] && pad=; done";
    const windows_command = "for /L %i in (1,1,1000000) do @echo " ++ flood ++ "%i";
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    const source = if (builtin.os.tag == .windows)
        try harness.createRuntimeWithHooksAndCommand(.{}, stand_in.executable, &.{ "/D", "/S", "/C", windows_command })
    else
        try harness.createRuntimeWithHooksAndCommand(.{}, "/bin/sh", &.{ "-c", posix_command });
    defer harness.destroyRuntimeWithIdentityRetry(source);
    // The shell's own flood is what this test exports.
    harness.deliverProgramOutput(source);

    var exports: usize = 0;
    var timer = try monotonic.Timer.start();
    while (exports < 5) {
        if (timer.read() >= output_timeout_ns) return error.ShellOutputTimedOut;
        monotonic.sleep(200 * std.time.ns_per_ms);
        try source.pauseReader();
        switch (source.awaitReaderParked(fixture.park_timeout_ns)) {
            .parked => {},
            // Output that reaches no ground within the bound refuses a quiesce as passing; this
            // test needs only exports, so it tries again.
            .mid_sequence => {
                try source.resumeReader();
                continue;
            },
            .timed_out, .stopped => |outcome| {
                std.debug.print("the busy shell's reader did not park: {s}\n", .{@tagName(outcome)});
                return error.ReaderNotParked;
            },
        }
        defer source.resumeReader() catch |err| std.debug.panic("resume failed: {s}", .{@errorName(err)});
        var capture = try screen_text.read(source, allocator, capture_limit_bytes);
        const busy = std.mem.indexOf(u8, capture.text(), flood) != null;
        capture.deinit();
        if (!busy) continue;
        const record = try source.exportHandover(allocator, fixture.inputs);
        defer allocator.free(record);
        var imported_output: ScriptedOutput = .{};
        var imported = try fixture.importScripted(record, &imported_output, &stand_in);
        defer imported.deinit();
        // The source stays parked, so both hold the exported state.
        try fixture.expectSameTerminal(source, imported.runtime);
        exports += 1;
    }
}

test "the corpus cut at every byte exports only at ground and each import fed the rest ends identical" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var source_output: ScriptedOutput = .{};
    const source = try fixture.createScripted(&source_output, &stand_in, 80, 24);
    defer source.destroy();

    var exported: usize = 0;
    var refused: usize = 0;
    for (1..cut_corpus.len) |cut| {
        source_output.commit(cut_corpus[0..cut]);
        try source.pauseReader();
        if (!groundAt(cut)) {
            source_output.awaitReadThrough();
            try std.testing.expectError(error.MidSequence, source.exportHandover(allocator, fixture.inputs));
            refused += 1;
            try source.resumeReader();
            source_output.commit(cut_corpus[cut..]);
            continue;
        }
        try std.testing.expectEqual(ReaderParkWait.parked, source.awaitReaderParked(fixture.park_timeout_ns));
        const record = try source.exportHandover(allocator, fixture.inputs);
        defer allocator.free(record);
        try source.resumeReader();
        exported += 1;

        var imported_output: ScriptedOutput = .{};
        var imported = try fixture.importScripted(record, &imported_output, &stand_in);
        defer imported.deinit();
        try fixture.expectSameTerminal(source, imported.runtime);
        source_output.commit(cut_corpus[cut..]);
        imported_output.commit(cut_corpus[cut..]);
        try fixture.expectSameTerminal(source, imported.runtime);
        try fixture.expectSameSections(source, imported.runtime);
    }
    std.debug.print("handover cut corpus: {d} bytes, {d} exports, {d} refusals\n", .{ cut_corpus.len, exported, refused });
    try std.testing.expect(exported > 0 and refused > 0);
}

test "a failed handover export is discarded and the reader reads on" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var output: ScriptedOutput = .{};
    const runtime = try fixture.createScripted(&output, &stand_in, 80, 24);
    defer runtime.destroy();
    output.commit("\x1b]133;P\x1b\\before-export\x1b]133;B\x1b\\\x1b]52;c;aGFuZG92ZXI=\x1b\\\r\n");
    try std.testing.expectError(error.NotParked, runtime.exportHandover(allocator, fixture.inputs));

    try fixture.park(runtime);
    const revision = harness.runtimeState(runtime).revision.load(.acquire);
    var failures: usize = 0;
    const record = while (true) : (failures += 1) {
        var failing: std.testing.FailingAllocator = .init(allocator, .{ .fail_index = failures });
        if (runtime.exportHandover(failing.allocator(), fixture.inputs)) |record| break record else |err| {
            try std.testing.expectEqual(error.OutOfMemory, err);
            try std.testing.expectEqual(failing.allocated_bytes, failing.freed_bytes);
        }
    };
    defer allocator.free(record);
    try std.testing.expect(failures > 0);
    try std.testing.expectEqual(revision, harness.runtimeState(runtime).revision.load(.acquire));
    try runtime.resumeReader();
    output.commit("after-export\r\n");

    var capture = try screen_text.read(runtime, allocator, capture_limit_bytes);
    defer capture.deinit();
    try std.testing.expect(std.mem.indexOf(u8, capture.text(), "before-export\nafter-export") != null);
}

test "a failed handover import is discarded and creates nothing" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var output: ScriptedOutput = .{};
    const source = try fixture.createScripted(&output, &stand_in, 80, 24);
    defer source.destroy();
    output.commit(cut_corpus ++ prompt ++ "\x1b]52;c;aGFuZG92ZXI=\x1b\\");
    try holdInput(source);
    const record = try fixture.exportParked(source);
    defer allocator.free(record);

    // Each allocation the import makes fails once; the last attempt fails only its first worker.
    var attempts: usize = 0;
    while (true) : (attempts += 1) {
        var failing: std.testing.FailingAllocator = .init(allocator, .{ .fail_index = attempts });
        var spawner: fixture.RefusingSpawner = .{};
        const result = subject.testing.importHandoverUsingSpawner(failing.allocator(), record, stand_in.launch(80, 24), &spawner);
        if (result) |_| return error.TestUnexpectedResult else |err| {
            try std.testing.expectEqual(failing.allocated_bytes, failing.freed_bytes);
            if (err == error.InjectedSpawnFailure) {
                try std.testing.expectEqual(@as(usize, 1), spawner.spawns);
                break;
            }
            try std.testing.expectEqual(error.OutOfMemory, err);
            try std.testing.expectEqual(@as(usize, 0), spawner.spawns);
        }
    }
    try std.testing.expect(attempts > 0);
}

const agent: harness.protocol.ProcessIdentity = .{ .pid = 4242, .creation_time = 77 };

/// Scripted reads, an Agent among the terminal's processes, and a clock the test moves.
const GatedOutput = struct {
    output: ScriptedOutput = .{},
    now: std.atomic.Value(u64) = .init(1000 * std.time.ns_per_s),

    fn hooks(self: *GatedOutput) harness.RuntimeTestingHooks {
        var result = self.output.hooks();
        result.has_process = hasProcess;
        result.monotonic_time_nanoseconds = monotonicTime;
        return result;
    }

    fn hasProcess(_: *anyopaque, identity: harness.protocol.ProcessIdentity) anyerror!bool {
        return identity.pid == agent.pid and identity.creation_time == agent.creation_time;
    }

    fn monotonicTime(context: *anyopaque) u64 {
        const output: *ScriptedOutput = @ptrCast(@alignCast(context));
        const self: *GatedOutput = @fieldParentPtr("output", output);
        return self.now.load(.monotonic);
    }
};

/// Enter for the Agent, on the terminal's current frame, for a prompt first seen at `first`.
fn gatedKey(runtime: *TerminalRuntime, first: u64) harness.protocol.ScreenGatedKeyInput {
    const runtime_state = harness.runtimeState(runtime);
    return .{ .key = .{
        .key = .enter,
        .process = agent,
        .generation = runtime_state.generation,
        .revision = runtime_state.revision.load(.acquire),
        .first_revision = first,
    } };
}

/// The bytes the gate typed for `input`.
fn typeGated(runtime: *TerminalRuntime, input: harness.protocol.ScreenGatedKeyInput) ![]const u8 {
    var prepared = try runtime.prepareScreenGatedKeyInput(input, std.math.maxInt(usize));
    defer prepared.deinit();
    const bytes = try allocator.dupe(u8, prepared.bytes orelse "");
    errdefer allocator.free(bytes);
    _ = try prepared.commit();
    return bytes;
}

test "screen gate: a moved terminal types no key decided before the move and answers a prompt first seen after it" {
    var stand_in: fixture.StandIn = try .init();
    defer stand_in.deinit();
    var source_output: ScriptedOutput = .{};
    const source = try fixture.createScripted(&source_output, &stand_in, 80, 24);
    defer source.destroy();
    source_output.commit("Do you want to proceed?\r\n> 1. Yes");
    // Decided on the old Host, on the frame where the prompt first showed.
    const decided = gatedKey(source, harness.runtimeState(source).revision.load(.acquire));
    const record = try fixture.exportParked(source);
    defer allocator.free(record);

    var gated: GatedOutput = .{};
    var spawner: harness.HookedThreadSpawner = .{ .hooks = gated.hooks() };
    const restored = try subject.testing.importHandoverUsingSpawner(allocator, record, stand_in.launch(80, 24), &spawner);
    gated.output.runtime = restored.runtime;
    var imported: fixture.Imported = .{ .runtime = restored.runtime, .facts = restored.facts };
    defer imported.deinit();

    // The move keeps the frame the key names, and a key decided on that screen after the move is
    // the same request: the prompt was first seen before the arrival either way.
    try std.testing.expectEqualDeep(decided, gatedKey(imported.runtime, decided.key.first_revision));
    try std.testing.expectError(error.ScreenPersonInput, typeGated(imported.runtime, decided));

    // A prompt first seen after the arrival waits out the quiet time from the arrival.
    gated.output.commit("\r\nDo you want to proceed?\r\n> 1. Yes");
    const first_seen = harness.runtimeState(imported.runtime).revision.load(.acquire);
    try std.testing.expectError(error.ScreenPersonRecent, typeGated(imported.runtime, gatedKey(imported.runtime, first_seen)));
    _ = gated.now.fetchAdd(screen_gate_quiet_ns, .monotonic);
    const typed = try typeGated(imported.runtime, gatedKey(imported.runtime, first_seen));
    defer allocator.free(typed);
    try std.testing.expectEqualSlices(u8, "\r", typed);
}
