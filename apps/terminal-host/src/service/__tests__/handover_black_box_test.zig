//! The H1 acceptance on real Windows, black-box: two processes of the built Host and its Keeper in
//! a runtime root the test owns, with the test as the Worker of all three and real programs on the
//! pinned conpty, which the runner stages beside this executable. A move keeps its program's
//! processes, screen, and history; after it, typed input and a resize reach the program through
//! the new Host, and closing the terminal there ends the program and its OpenConsole. Moves during
//! output whose sequences cross reads park only at ground, and one while a program stops inside
//! its own sequence is refused as passing and leaves the terminal on its old Host. A failure forced
//! at every step of a move leaves the terminal reading on its old Host with every handle count back
//! at its baseline, and ending the old Host after the quiesce, or the Keeper at any step, leaves
//! the program running. A run that asks for it reports the pause from the quiesce to the claim
//! during a flood, at the default scrollback, and at 100 MB of history. The test reads screens and
//! history only as a Worker does: through attachments, their snapshot frames, and the plain export
//! of select-all.

const std = @import("std");
const moves = @import("handover-move-support");
const options = @import("handover_black_box_options");

const Fixture = moves.Fixture;
const MessageKind = moves.MessageKind;
const TerminalId = moves.TerminalId;
const Side = moves.Side;
const Baseline = moves.Baseline;
const ms = moves.ms;
const report = moves.report;
const expectKind = moves.expectKind;
const expectFailure = moves.expectFailure;
const expectOutcome = moves.expectOutcome;

const frame_header_bytes = 160;
const export_chunk_header_bytes = 32;
const max_record_chunk_bytes = 4 * 1024 * 1024;
/// A scrollback that keeps every line the numbered-line probe prints during the failure matrix.
const matrix_scrollback_bytes = 64_000_000;

// ---- the probe and its terminals ----

fn hex(bytes: [16]u8) [32]u8 {
    return std.fmt.bytesToHex(bytes, .lower);
}

/// The program and OpenConsole of a terminal, as the Keeper names them.
const Processes = struct { program: moves.message.Process, console: moves.message.Process };

fn processesOf(f: *Fixture, id: TerminalId, owner: Side) !Processes {
    const entry = try f.awaitHolding(id, f.identity(owner), 5000);
    return .{ .program = entry.facts.terminal.program, .console = entry.facts.terminal.console };
}

/// Requires that the Keeper holds the terminal for `owner` with the same program and OpenConsole.
fn expectProcesses(f: *Fixture, id: TerminalId, owner: Side, expected: Processes) !void {
    const actual = try processesOf(f, id, owner);
    if (actual.program.eql(expected.program) and actual.console.eql(expected.console)) return;
    report("the moved terminal names program {d} and console {d}, created as {d} and {d}", .{ actual.program.pid, actual.console.pid, expected.program.pid, expected.console.pid });
    return error.ProcessChanged;
}

// ---- attachments ----

/// A terminal's screen and history as a Worker reads them on one Host.
const View = struct {
    /// The first snapshot frame of a new attachment, with the fields that name the Host, the
    /// attachment, its delivery, and the revisions zeroed.
    frame: []const u8,
    /// The plain export of select-all: every line of the history and the screen.
    text: []const u8,
    /// The frame's retained scrollback rows.
    history_rows: u64,
};

/// Reads the terminal's view on `side` through an attachment of its own.
fn capture(f: *Fixture, side: Side, id: TerminalId, with_text: bool) !View {
    const attachment = try f.attachTerminal(side, id, .{ .read = true });
    const first = f.records.items.len;
    const demand = try std.fmt.allocPrint(f.arena(), "{{\"attachmentId\":\"{s}\",\"demand\":\"active\"}}", .{&hex(attachment)});
    try expectKind(try f.call(side, MessageKind.set_projection_demand, .control_json, demand, 10_000), MessageKind.projection_demand_accepted);
    const frame = try awaitSnapshot(f, side, attachment, first);
    const text = if (with_text) try exportAll(f, side, attachment, std.mem.readInt(u64, frame[72..80], .little)) else "";
    try f.detachTerminal(side, attachment);
    const masked = try f.arena().dupe(u8, frame);
    @memset(masked[16..80], 0);
    @memset(masked[145..153], 0);
    return .{ .frame = masked, .text = text, .history_rows = std.mem.readInt(u64, frame[80..88], .little) };
}

fn awaitSnapshot(f: *Fixture, side: Side, attachment: [16]u8, first: usize) ![]const u8 {
    const deadline = f.timer.read() + 10_000 * ms;
    while (true) {
        for (f.records.items[first..]) |record| {
            if (record.side != side or !record.event or record.kind != MessageKind.snapshot_frame) continue;
            if (record.payload.len < frame_header_bytes or !std.mem.eql(u8, record.payload[32..48], &attachment)) continue;
            return record.payload;
        }
        if (f.timer.read() > deadline) return error.SnapshotMissing;
        try f.tick();
    }
}

/// Selects everything and returns its plain export, acknowledging each chunk.
fn exportAll(f: *Fixture, side: Side, attachment: [16]u8, generation: u64) ![]const u8 {
    const select = try std.fmt.allocPrint(f.arena(), "{{\"attachmentId\":\"{s}\",\"command\":\"select-all\",\"generation\":\"{d}\",\"revision\":\"0\"}}", .{ &hex(attachment), generation });
    try expectKind(try f.call(side, MessageKind.selection_command, .control_json, select, 10_000), MessageKind.selection_changed);
    var scanned = f.records.items.len;
    const start = try std.fmt.allocPrint(f.arena(), "{{\"attachmentId\":\"{s}\",\"exportSequence\":\"1\",\"format\":\"plain\"}}", .{&hex(attachment)});
    try expectKind(try f.call(side, MessageKind.selection_export, .control_json, start, 10_000), MessageKind.selection_export_started);
    var text: std.ArrayList(u8) = .empty;
    const deadline = f.timer.read() + 60_000 * ms;
    while (true) {
        while (scanned < f.records.items.len) : (scanned += 1) {
            const record = f.records.items[scanned];
            if (record.side != side or !record.event or record.kind != MessageKind.selection_export_chunk) continue;
            if (record.payload.len < export_chunk_header_bytes or !std.mem.eql(u8, record.payload[0..16], &attachment)) continue;
            try text.appendSlice(f.arena(), record.payload[export_chunk_header_bytes..]);
            const sequence = std.mem.readInt(u32, record.payload[24..28], .little);
            const applied = try std.fmt.allocPrint(f.arena(), "{{\"attachmentId\":\"{s}\",\"chunkSequence\":{d},\"exportSequence\":\"1\"}}", .{ &hex(attachment), sequence });
            try expectKind(try f.call(side, MessageKind.selection_export_chunk_applied, .control_json, applied, 10_000), MessageKind.selection_export_chunk_applied);
            if (record.payload[28] & 1 != 0) return text.items;
        }
        if (f.timer.read() > deadline) return error.ExportUnfinished;
        try f.tick();
    }
}

/// Resizes the terminal on `side` and waits until its program reports the new size.
fn resizeAndAwait(f: *Fixture, side: Side, id: TerminalId, columns: u16, rows: u16) !void {
    const attachment = try f.attachTerminal(side, id, .{ .resize = true });
    const payload = try std.fmt.allocPrint(f.arena(), "{{\"attachmentId\":\"{s}\",\"cellPixelHeight\":16,\"cellPixelWidth\":8,\"columns\":{d},\"rows\":{d}}}", .{ &hex(attachment), columns, rows });
    try expectKind(try f.call(side, MessageKind.resize_terminal, .control_json, payload, 10_000), MessageKind.resize_accepted);
    _ = try f.awaitTitle(side, id, try std.fmt.allocPrint(f.arena(), "size-{d}x{d}", .{ columns, rows }), false, 20_000);
    try f.detachTerminal(side, attachment);
}

/// Closes the terminal on `side`: its program and OpenConsole end, and it is disposed.
fn close(f: *Fixture, side: Side, id: TerminalId) !void {
    try expectKind(try f.terminate(side, id), MessageKind.terminate_terminal);
    try f.awaitEnded(id, 10_000);
    _ = try f.awaitExitCode(side, id, 10_000);
    try f.dispose(side, id);
    f.unwatch(id);
}

// ---- moves ----

/// A whole move to the other Host: claimed there, the old copy dropped. Returns the terminal's
/// identity on the new Host.
fn move(f: *Fixture, from: Side, id: TerminalId) !TerminalId {
    const copy = try f.moveUntilDrop(from, from.other(), id);
    try expectKind(try f.dropOn(from, id), MessageKind.drop_terminal);
    try f.awaitGone(from, id, 5000);
    return copy;
}

/// Requires the same screen and the same history in both views.
fn expectSameView(before: View, after: View) !void {
    if (!std.mem.eql(u8, before.frame, after.frame)) {
        const length = @min(before.frame.len, after.frame.len);
        const at = for (0..length) |index| {
            if (before.frame[index] != after.frame[index]) break index;
        } else length;
        report("the snapshot changed: {d} bytes before, {d} after, first difference at byte {d}", .{ before.frame.len, after.frame.len, at });
        return error.ScreenChanged;
    }
    if (!std.mem.eql(u8, before.text, after.text)) {
        const at = std.mem.indexOfDiff(u8, before.text, after.text) orelse 0;
        const from = at -| 40;
        report("the history changed at byte {d}: before \"{s}\", after \"{s}\"", .{ at, before.text[from..@min(before.text.len, at + 40)], after.text[from..@min(after.text.len, at + 40)] });
        return error.HistoryChanged;
    }
}

/// Whether `expected` are consecutive lines of `text`, each without its trailing blanks.
fn hasLines(f: *Fixture, text: []const u8, expected: []const []const u8) !bool {
    var lines: std.ArrayList([]const u8) = .empty;
    var split = std.mem.splitScalar(u8, text, '\n');
    while (split.next()) |raw| try lines.append(f.arena(), std.mem.trimEnd(u8, raw, "\r "));
    if (lines.items.len < expected.len) return false;
    for (0..lines.items.len - expected.len + 1) |start| {
        for (expected, lines.items[start..][0..expected.len]) |want, line| {
            if (!std.mem.eql(u8, want, line)) break;
        } else return true;
    }
    return false;
}

/// Requires the flood's lines from the oldest the history kept to `last` once each, in order, and
/// each exactly as the probe wrote it; returns the oldest.
fn expectFloodLines(text: []const u8, last: u64) !u64 {
    const padding = "abcdefghijklmnopqrstuvwxyz0123456789";
    var first: ?u64 = null;
    var expected: u64 = 0;
    var lines = std.mem.splitScalar(u8, text, '\n');
    while (lines.next()) |raw| {
        const line = std.mem.trimEnd(u8, raw, "\r ");
        if (!std.mem.startsWith(u8, line, "flood-") or std.mem.startsWith(u8, line, "flood-end")) continue;
        const space = std.mem.indexOfScalar(u8, line, ' ') orelse line.len;
        const number = std.fmt.parseInt(u64, line["flood-".len..space], 10) catch {
            report("a damaged flood line: \"{s}\"", .{line});
            return error.LineDamaged;
        };
        if (first == null) {
            first = number;
            expected = number;
        }
        var buffer: [96]u8 = undefined;
        const intact = try std.fmt.bufPrint(&buffer, "flood-{d} {s}", .{ number, padding[0 .. 1 + number % 29] });
        if (number != expected or !std.mem.eql(u8, line, intact)) {
            report("\"{s}\" where flood-{d} was expected", .{ line, expected });
            return error.LineLostOrRepeated;
        }
        expected += 1;
    }
    if (first != null and expected == last + 1) return first.?;
    report("the history ends at flood-{d}, the probe printed {d}", .{ expected -| 1, last });
    return error.LineLostOrRepeated;
}

// ---- the witnesses ----

test "a numbered-line probe and a full-screen program move with their processes, screens, and history" {
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const before = try f.counts();

    // The numbered-line probe moves four times while it prints, and once more at rest.
    var lines = try f.createTerminal(.a);
    var owner: Side = .a;
    const lines_processes = try processesOf(f, lines, owner);
    try f.idle(300);
    for (0..4) |_| {
        lines = try move(f, owner, lines);
        owner = owner.other();
        try expectProcesses(f, lines, owner, lines_processes);
        try f.idle(250);
    }
    const printed = try moves.titleNumber(try f.typeAndAwait(owner, lines, "x", "lines-end ", true));
    try f.idle(200);
    const at_rest = try capture(f, owner, lines, true);
    lines = try move(f, owner, lines);
    owner = owner.other();
    try expectProcesses(f, lines, owner, lines_processes);
    const moved = try capture(f, owner, lines, true);
    try expectSameView(at_rest, moved);
    try moves.expectNumberedLines(moved.text, printed);
    report("a numbered-line probe moved five times, four while it printed: line-1 to line-{d} each once, the screen and {d} history rows unchanged by the last move", .{ printed, moved.history_rows });

    // After the move, input and a resize reach the program through the new Host, and closing it
    // there ends the program and OpenConsole.
    _ = try f.typeAndAwait(owner, lines, "k", "key-k", false);
    try resizeAndAwait(f, owner, lines, 100, 30);
    try close(f, owner, lines);

    // A full-screen program on the alternate screen moves the same way.
    const screen = try f.createTerminalWith(.a, f.probe, &.{"screen"});
    _ = try f.awaitTitle(.a, screen, "screen-ready", false, 10_000);
    const screen_processes = try processesOf(f, screen, .a);
    try f.idle(300);
    const drawn = try capture(f, .a, screen, true);
    const screen_on_b = try move(f, .a, screen);
    try expectProcesses(f, screen_on_b, .b, screen_processes);
    try expectSameView(drawn, try capture(f, .b, screen_on_b, true));
    _ = try f.typeAndAwait(.b, screen_on_b, "k", "key-k", false);
    try resizeAndAwait(f, .b, screen_on_b, 100, 30);
    try close(f, .b, screen_on_b);
    report("a full-screen program moved with its screen unchanged; input, a resize, and closing acted through the new Host", .{});
    try f.expectCounts(before.all(), 10_000);
}

test "moves during output whose sequences cross reads park only at ground and lose or repeat no line" {
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const before = try f.counts();
    var id = try f.createTerminalWith(.a, f.probe, &.{"flood"});
    var owner: Side = .a;
    try f.idle(300);
    var moved: usize = 0;
    var passed: usize = 0;
    // A run that measures the pause also times these moves, whose quiesce applies the last batch
    // the reader took, from the quiesce request to the claim's answer.
    var pauses: [6]u64 = undefined;
    while (moved < pauses.len) {
        f.spin = options.measure_pause;
        const begin = f.timer.read();
        const answer = try f.quiesce(owner, id, f.identity(owner.other()), 30);
        if (answer.kind == MessageKind.typed_failure) {
            f.spin = false;
            // Output that reaches no ground within the bound refuses the quiesce as passing.
            try expectFailure(answer, "move-passing", null);
            passed += 1;
            if (passed > 30) return error.FloodNeverParked;
            try f.idle(100);
            continue;
        }
        try expectKind(answer, MessageKind.quiesce_terminal);
        const Quiesced = struct { recordBytes: u32 };
        const bytes = (try std.json.parseFromSliceLeaky(Quiesced, f.arena(), answer.payload, .{ .ignore_unknown_fields = true })).recordBytes;
        const copy = try f.restored(owner, owner.other(), id, bytes, 30);
        try expectKind(try f.claim(owner.other(), copy), MessageKind.claim_terminal);
        pauses[moved] = f.timer.read() - begin;
        f.spin = false;
        try f.resumed(owner, id, "claimed-elsewhere");
        try expectKind(try f.dropOn(owner, id), MessageKind.drop_terminal);
        try f.awaitGone(owner, id, 5000);
        id = copy;
        owner = owner.other();
        moved += 1;
        try f.idle(200);
    }
    const printed = try moves.titleNumber(try f.typeAndAwait(owner, id, "x", "flood-end ", true));
    try f.idle(300);
    const view = try capture(f, owner, id, true);
    const oldest = try expectFloodLines(view.text, printed);
    report("{d} moves during a flood, {d} quiesces refused as passing; flood-{d} to flood-{d} each once and intact", .{ moved, passed, oldest, printed });
    if (options.measure_pause) {
        std.mem.sort(u64, &pauses, {}, std.sort.asc(u64));
        report("pause during the flood: median {d:.1} ms, worst {d:.1} ms over {d} moves", .{ nsMs((pauses[2] + pauses[3]) / 2), nsMs(pauses[5]), pauses.len });
    }
    try close(f, owner, id);
    try f.expectCounts(before.all(), 10_000);
}

test "a move while a program stops inside its own sequence is refused as passing and leaves the terminal on its old Host" {
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const before = try f.counts();
    const id = try f.createTerminalWith(.a, f.probe, &.{"stall"});
    const processes = try processesOf(f, id, .a);
    _ = try f.awaitTitle(.a, id, "stall-begin", false, 10_000);
    try f.idle(300);
    // The pinned conpty passes the program's partial sequence through, so the reader stops inside
    // it, and no quiesce parks while the program waits for a key.
    const refusals = 3;
    for (0..refusals) |_| {
        try expectFailure(try f.quiesce(.a, id, f.identity(.b), 30), "move-passing", "mid-sequence");
        try f.awaitHandoverState(.a, id, null, 1000);
        try f.idle(200);
    }
    try expectProcesses(f, id, .a, processes);
    try f.expectCounts(.{ .b = before.b }, 5000);

    // The terminal reads on its old Host, where the key completes the sequence.
    _ = try f.typeAndAwait(.a, id, "g", "stall-end", false);
    try f.idle(200);
    const completed = try capture(f, .a, id, true);
    if (!try hasLines(f, completed.text, &.{ "stall-begin", "stall-red", "stall-end" }) or
        std.mem.indexOf(u8, completed.text, "[3") != null or std.mem.indexOf(u8, completed.text, "1mstall") != null)
    {
        report("the completed sequence shows as \"{s}\"", .{completed.text});
        return error.SequenceDamaged;
    }

    // At ground again, the terminal moves with its screen and history.
    const copy = try move(f, .a, id);
    try expectProcesses(f, copy, .b, processes);
    try expectSameView(completed, try capture(f, .b, copy, true));
    try close(f, .b, copy);
    report("{d} quiesces while the program stopped inside its sequence refused as mid-sequence; the terminal read on, completed the sequence, and moved at ground", .{refusals});
    try f.expectCounts(before.all(), 10_000);
}

const Step = enum { quiesce, read, import, write, restore, claim };
const Failure = enum { abandon, end_new_host, end_keeper, end_old_host };

/// A move under way, as far as the Worker has taken it.
const Move = struct {
    from: Side,
    id: TerminalId,
    bytes: u32 = 0,
    record: []const u8 = &.{},
    creation: [16]u8 = undefined,
    copy: ?TerminalId = null,

    fn to(self: *const Move) Side {
        return self.from.other();
    }
};

fn advance(f: *Fixture, m: *Move, step: Step) !void {
    switch (step) {
        .quiesce => m.bytes = try f.quiesced(m.from, m.id, m.to(), 30),
        .read => m.record = try f.readRecord(m.from, m.id, m.bytes),
        .import => {
            m.creation = f.creationId();
            try expectKind(try f.call(m.to(), MessageKind.import_terminal, .control_json, try f.importPayload(m.creation, m.record.len, 30), 10_000), MessageKind.import_terminal);
        },
        .write => {
            var offset: usize = 0;
            while (offset < m.record.len) {
                const end = @min(m.record.len, offset + max_record_chunk_bytes);
                try expectKind(try f.call(m.to(), MessageKind.write_handover_record, .handover_record, try f.chunkPayload(m.creation, offset, m.record[offset..end]), 10_000), MessageKind.handover_record_written);
                offset = end;
            }
        },
        .restore => {
            const identity = try f.restore(m.to(), m.creation);
            try expectKind(identity, MessageKind.restore_terminal);
            m.copy = try f.copyOf(m.id, identity.payload);
        },
        .claim => try expectKind(try f.claim(m.to(), m.copy.?), MessageKind.claim_terminal),
    }
}

/// The new Host's discard of whatever import the move made there.
fn discardImport(f: *Fixture, m: *const Move, last: Step) !void {
    const payload = switch (last) {
        .quiesce, .read => return,
        .import, .write => try f.creationPayload(m.creation),
        .restore => try f.targetPayload(m.copy.?),
        .claim => unreachable,
    };
    const answer = try f.discard(m.to(), payload);
    try expectKind(answer, MessageKind.discard_import);
    try expectOutcome(f.allocator, answer.payload, "discarded");
    if (m.copy) |copy| try f.awaitGone(m.to(), copy, 5000);
}

/// A Host restarted after its end, warmed up as at the start.
fn restartHost(f: *Fixture, side: Side) !void {
    try f.startHost(side);
    try f.warmUp(&.{side});
}

const Settled = struct { id: TerminalId, owner: Side };

/// Forces `failure` once the move has taken `last`, then settles the move as the Worker would.
fn force(f: *Fixture, m: *Move, last: Step, failure: Failure) !Settled {
    switch (failure) {
        .abandon => {
            try discardImport(f, m, last);
            try f.resumed(m.from, m.id, "resumed");
            return .{ .id = m.id, .owner = m.from };
        },
        .end_new_host => {
            f.killHost(m.to());
            try std.testing.expect(f.running(m.id));
            try f.resumed(m.from, m.id, "resumed");
            try restartHost(f, m.to());
            return .{ .id = m.id, .owner = m.from };
        },
        .end_keeper => {
            try f.loseKeeper();
            try std.testing.expect(f.running(m.id));
            try f.startKeeper();
            try f.attach(.a);
            try f.attach(.b);
            if (last == .claim) {
                // The claim committed: the new Host reads, and the old copy, still stopped, drops.
                try expectKind(try f.dropOn(m.from, m.id), MessageKind.drop_terminal);
                try f.awaitGone(m.from, m.id, 5000);
                return .{ .id = m.copy.?, .owner = m.to() };
            }
            try discardImport(f, m, last);
            try f.resumed(m.from, m.id, "resumed");
            return .{ .id = m.id, .owner = m.from };
        },
        .end_old_host => {
            // Nothing was read after the export, so the new Host still claims the terminal; after
            // the claim nothing changes.
            f.killHost(m.from);
            try std.testing.expect(f.running(m.id));
            var step = @intFromEnum(last) + 1;
            while (step <= @intFromEnum(Step.claim)) : (step += 1) try advance(f, m, @enumFromInt(step));
            try restartHost(f, m.from);
            return .{ .id = m.copy.?, .owner = m.to() };
        },
    }
}

fn countOf(counts: moves.Counts, side: Side) u32 {
    return if (side == .a) counts.a else counts.b;
}

fn withHost(baseline: Baseline, side: Side, count: u32) Baseline {
    var result = baseline;
    if (side == .a) result.a = count else result.b = count;
    return result;
}

test "a failure forced at every step of a move leaves the terminal on its old Host with every handle back" {
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const start = try f.counts();
    // Each Host's count while it serves no terminal; a restarted Host takes a new one.
    var idle = [2]u32{ start.a, start.b };
    var id = try f.createTerminalLaunched(.a, f.probe, &.{"lines"}, .{ .scrollback_bytes = matrix_scrollback_bytes });
    var owner: Side = .a;
    const processes = try processesOf(f, id, owner);
    const cases = [_]struct { failure: Failure, first: Step, last: Step }{
        .{ .failure = .abandon, .first = .quiesce, .last = .restore },
        .{ .failure = .end_new_host, .first = .quiesce, .last = .restore },
        .{ .failure = .end_keeper, .first = .quiesce, .last = .claim },
        // The record lives only in the old Host until the Worker reads it.
        .{ .failure = .end_old_host, .first = .read, .last = .claim },
    };
    var forced: usize = 0;
    for (cases) |case| {
        var last = case.first;
        while (true) : (last = @enumFromInt(@intFromEnum(last) + 1)) {
            try f.idle(150);
            const held = try f.counts();
            var m: Move = .{ .from = owner, .id = id };
            var step: Step = .quiesce;
            while (true) : (step = @enumFromInt(@intFromEnum(step) + 1)) {
                try advance(f, &m, step);
                if (step == last) break;
            }
            const settled = try force(f, &m, last, case.failure);
            try expectProcesses(f, settled.id, settled.owner, processes);
            const other = settled.owner.other();
            switch (case.failure) {
                .abandon => try f.expectCounts(held.all(), 10_000),
                .end_new_host => {
                    try f.expectCounts(withHost(.{ .keeper = held.keeper }, owner, countOf(held, owner)), 10_000);
                    idle[@intFromEnum(other)] = countOf(try f.counts(), other);
                },
                .end_keeper => if (settled.owner == owner) {
                    try f.expectCounts(.{ .a = held.a, .b = held.b }, 10_000);
                } else {
                    // The old Host dropped its copy and serves nothing again.
                    try f.expectCounts(withHost(.{}, other, idle[@intFromEnum(other)]), 10_000);
                },
                .end_old_host => {
                    try f.expectCounts(.{ .keeper = held.keeper }, 10_000);
                    idle[@intFromEnum(other)] = countOf(try f.counts(), other);
                },
            }
            report("{s} after {s}: the terminal reads on Host {s}", .{ @tagName(case.failure), @tagName(last), @tagName(settled.owner) });
            id = settled.id;
            owner = settled.owner;
            forced += 1;
            if (last == case.last) break;
        }
    }

    // Every line the probe printed through the forced failures is in the history once.
    const printed = try moves.titleNumber(try f.typeAndAwait(owner, id, "x", "lines-end ", true));
    try f.idle(200);
    try moves.expectNumberedLines((try capture(f, owner, id, true)).text, printed);
    try close(f, owner, id);
    try f.expectCounts(.{ .a = idle[0], .b = idle[1] }, 10_000);
    report("{d} forced failures: line-1 to line-{d} each once; both Hosts back at their counts", .{ forced, printed });
}

test "ending the old Host right after the quiesce leaves the program running, held by the Keeper" {
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const before = try f.counts();
    const id = try f.createTerminal(.a);
    const processes = try processesOf(f, id, .a);
    const old_host = f.identity(.a);
    try f.idle(300);
    _ = try f.quiesced(.a, id, .b, 30);
    // The record lived only in Host A, so the move ends with it, and the Keeper holds the program.
    f.killHost(.a);
    const deadline = f.timer.read() + 5000 * ms;
    while (true) {
        const entry = (try f.holdingOf(id)) orelse return error.TerminalNotHeld;
        if (entry.state == .orphaned and entry.owner.eql(old_host)) {
            try std.testing.expect(entry.facts.terminal.program.eql(processes.program) and entry.facts.terminal.console.eql(processes.console));
            break;
        }
        if (f.timer.read() > deadline) {
            report("the holding is {s} for process {d}", .{ entry.state.name(), entry.owner.pid });
            return error.HoldingNotOrphaned;
        }
        try f.idle(20);
    }
    try f.idle(1000);
    try std.testing.expect(f.running(id));
    try f.expectCounts(.{ .b = before.b }, 5000);
    report("the old Host ended right after the quiesce: the program and OpenConsole run on in an orphaned holding", .{});
}

/// The launch profile a recovery names, as the Worker resolves it.
const recovery_profile = "{\"colorScheme\":\"dark\",\"defaultBackgroundRgb\":0,\"defaultCursorRgb\":null,\"defaultForegroundRgb\":16777215,\"deviceAttributes\":{\"primaryConformanceLevel\":62,\"primaryFeatures\":[22],\"secondaryDeviceType\":1,\"secondaryFirmwareVersion\":0,\"secondaryRomCartridge\":0,\"tertiaryUnitId\":0},\"enquiryResponse\":\"\",\"palette\":[" ++ ("0," ** 255) ++ "0],\"versionResponse\":\"Orchbee test\"}";

test "a program whose Host ended runs on and comes back on a recovery Host of the same epoch" {
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const id = try f.createTerminalWith(.a, f.probe, &.{"screen"});
    _ = try f.awaitTitle(.a, id, "screen-ready", false, 10_000);
    const processes = try processesOf(f, id, .a);
    const lost = f.identity(.a);
    const epoch = f.epochOf(.a);
    f.killHost(.a);
    const deadline = f.timer.read() + 5000 * ms;
    const orphan = while (true) {
        const entry = (try f.holdingOf(id)) orelse return error.TerminalNotHeld;
        if (entry.state == .orphaned and entry.owner.eql(lost)) break entry;
        if (f.timer.read() > deadline) return error.HoldingNotOrphaned;
        try f.idle(20);
    };
    try std.testing.expect(f.running(id));

    // The recovery Host takes the lost Host's epoch, so the terminal keeps its identity.
    f.killHost(.b);
    try f.startHostWith(.b, epoch);
    var mark: [16]u8 = undefined;
    f.io.random(&mark);
    try f.assign(orphan.holding, f.identity(.b), mark);
    const payload = try std.fmt.allocPrint(f.arena(), "{{\"creationId\":\"{s}\",\"deadlineSeconds\":30,\"geometry\":{{\"cellPixelHeight\":16,\"cellPixelWidth\":8,\"columns\":100,\"rows\":30}},\"holding\":\"{s}\",\"mark\":\"{s}\",\"terminalId\":\"{s}\",\"terminalProfile\":{s}}}", .{
        &hex(orphan.facts.terminal.terminal.creation_id), &hex(orphan.holding), &hex(mark), &hex(id.bytes), recovery_profile,
    });
    const recovered = try f.call(.b, MessageKind.recover_terminal, .control_json, payload, 20_000);
    try expectKind(recovered, MessageKind.recover_terminal);
    try std.testing.expect((try moves.terminalIdOf(f.allocator, recovered.payload)).eql(id));
    // The claim reads nothing before it: the program's screen comes back with the console's resize.
    try expectKind(try f.claim(.b, id), MessageKind.claim_terminal);
    try expectProcesses(f, id, .b, processes);
    _ = try f.awaitTitle(.b, id, "size-100x30", false, 20_000);
    _ = try f.typeAndAwait(.b, id, "k", "key-k", false);
    try resizeAndAwait(f, .b, id, 90, 28);
    try close(f, .b, id);
    report("the program outlived its Host in the Keeper, and a recovery Host of the same epoch claimed it under its identity: input, a resize, and closing acted there", .{});
}

/// One move whose steps are timed, from the quiesce request to the claim's answer, which bounds
/// the time the program's output is not read. The waits spin, so no step waits out a timer tick.
fn timedMove(f: *Fixture, from: Side, id: TerminalId, name: []const u8, repetition: usize, rows: u64) !struct { copy: TerminalId, total_ns: u64 } {
    const to = from.other();
    f.spin = true;
    defer f.spin = false;
    const begin = f.timer.read();
    const bytes = try f.quiesced(from, id, to, 30);
    const quiesced_at = f.timer.read();
    const record = try f.readRecord(from, id, bytes);
    const read_at = f.timer.read();
    const creation = f.creationId();
    try f.importRecord(to, creation, record, 30);
    const imported_at = f.timer.read();
    const identity = try f.restore(to, creation);
    try expectKind(identity, MessageKind.restore_terminal);
    const restored_at = f.timer.read();
    const copy = try f.copyOf(id, identity.payload);
    try expectKind(try f.claim(to, copy), MessageKind.claim_terminal);
    const claimed_at = f.timer.read();
    try f.resumed(from, id, "claimed-elsewhere");
    try expectKind(try f.dropOn(from, id), MessageKind.drop_terminal);
    try f.awaitGone(from, id, 5000);
    report("pause at {s}, move {d}: {d} history rows, record {d} bytes; quiesce {d:.1} ms, read {d:.1} ms, import {d:.1} ms, restore {d:.1} ms, claim {d:.1} ms; quiesce to claim {d:.1} ms", .{
        name,                           repetition,                  rows,                        bytes,
        nsMs(quiesced_at - begin),      nsMs(read_at - quiesced_at), nsMs(imported_at - read_at), nsMs(restored_at - imported_at),
        nsMs(claimed_at - restored_at), nsMs(claimed_at - begin),
    });
    return .{ .copy = copy, .total_ns = claimed_at - begin };
}

fn nsMs(ns: u64) f64 {
    return @as(f64, @floatFromInt(ns)) / std.time.ns_per_ms;
}

test "the pause from the quiesce to the claim at the default scrollback and at 100 MB of history" {
    // A measurement, whose report is its result: only `-Dhandover-pause` runs it.
    if (!options.measure_pause) return error.SkipZigTest;
    var f = try Fixture.createWith(.process);
    defer f.destroy();
    const cases = [_]struct { name: []const u8, scrollback_bytes: ?usize, lines: u32 }{
        // More lines than each bound keeps, so the history is full; 120 × 40, as prototype 2.
        .{ .name = "the default scrollback", .scrollback_bytes = null, .lines = 16_000 },
        .{ .name = "100 MB", .scrollback_bytes = 100_000_000, .lines = 160_000 },
    };
    for (cases) |case| {
        var count_buffer: [16]u8 = undefined;
        const count = try std.fmt.bufPrint(&count_buffer, "{d}", .{case.lines});
        var id = try f.createTerminalLaunched(.a, f.probe, &.{ "fill", count }, .{ .columns = 120, .rows = 40, .scrollback_bytes = case.scrollback_bytes });
        const fill_begin = f.timer.read();
        _ = try f.awaitTitle(.a, id, try std.fmt.allocPrint(f.arena(), "fill-end {d}", .{case.lines}), false, 400_000);
        try f.idle(500);
        const rows = (try capture(f, .a, id, false)).history_rows;
        report("filled {s} with {d} lines in {d:.1} s: {d} history rows", .{ case.name, case.lines, nsMs(f.timer.read() - fill_begin) / 1000, rows });
        var owner: Side = .a;
        var totals: [3]u64 = undefined;
        for (&totals, 1..) |*total, repetition| {
            const timed = try timedMove(f, owner, id, case.name, repetition, rows);
            total.* = timed.total_ns;
            id = timed.copy;
            owner = owner.other();
        }
        std.mem.sort(u64, &totals, {}, std.sort.asc(u64));
        report("pause at {s}: median {d:.1} ms, worst {d:.1} ms over {d} moves", .{ case.name, nsMs(totals[1]), nsMs(totals[2]), totals.len });
        try close(f, owner, id);
    }
}
