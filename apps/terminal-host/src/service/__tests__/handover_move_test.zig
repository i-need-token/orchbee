//! Moves of a running terminal between two Host processes through a real Keeper, with a failure
//! forced at each seam of a move: the Keeper refuses a request, stops answering, or ends, a
//! deadline passes, or a Host ends. Each witness checks that at most one Host reads, that the
//! program and OpenConsole outlive every move and end with the terminal on its owner, and that the
//! handle counts of both Hosts and the Keeper return to their baselines. Host A is this process, so
//! a terminal that reads on there, or moves there, shows each line its program printed once and in
//! order. A restored copy has its own identity on the new Host, named `copy` here.

const std = @import("std");
const support = @import("handover-move-support");

// The black-box moves, whose Host A is a process too, run in this executable, so the package test
// compiles the Host's test graph once for both.
test {
    _ = @import("handover_black_box_test.zig");
}

const Fixture = support.Fixture;
const MessageKind = support.MessageKind;
const TerminalId = support.TerminalId;
const expectKind = support.expectKind;
const expectFailure = support.expectFailure;
const expectOutcome = support.expectOutcome;

/// Stops the probe on Host A and requires every line it printed there once and in order; then the
/// probe ends by itself when told to, and its program and OpenConsole end with it.
fn expectReadOn(f: *Fixture, id: TerminalId) !void {
    const printed = try support.titleNumber(try f.typeAndAwait(.a, id, "x", "lines-end ", true));
    const text = try f.awaitLine(id, try std.fmt.allocPrint(f.arena(), "lines-end {d}", .{printed}), 10_000);
    try support.expectNumberedLines(text, printed);
    try f.typeInput(.a, id, "q");
    try std.testing.expectEqual(@as(?u32, 0), try f.awaitExitCode(.a, id, 10_000));
    try f.awaitEnded(id, 10_000);
}

fn expectDiscarded(f: *Fixture, record: support.Record) !void {
    try expectKind(record, MessageKind.discard_import);
    try expectOutcome(f.allocator, record.payload, "discarded");
}

fn endOn(f: *Fixture, side: support.Side, ids: []const TerminalId) !void {
    for (ids) |id| try expectKind(try f.terminate(side, id), MessageKind.terminate_terminal);
    for (ids) |id| try f.awaitEnded(id, 10_000);
}

test "a move claims on the new Host, drops the old copy, and loses or repeats no output" {
    var f = try Fixture.create();
    defer f.destroy();
    const before = try f.counts();
    const id = try f.createTerminal(.b);
    const holding = f.terminal(id).holding;

    // Quiesced, the old Host refuses termination and, while its Keeper answers, a drop.
    const bytes = try f.quiesced(.b, id, .a, 30);
    try f.awaitHandoverState(.b, id, "quiesced", 1000);
    try expectFailure(try f.terminate(.b, id), "terminal-quiesced", null);
    try expectFailure(try f.dropOn(.b, id), "move-passing", "moving");

    // Restored, the new Host reads nothing and refuses termination until its claim.
    const copy = try f.restored(.b, .a, id, bytes, 30);
    try f.awaitHandoverState(.a, copy, "restored", 1000);
    try expectFailure(try f.terminate(.a, copy), "terminal-quiesced", null);

    try expectKind(try f.claim(.a, copy), MessageKind.claim_terminal);
    try f.awaitHandoverState(.a, copy, null, 1000);
    const entry = try f.awaitHolding(copy, f.identity(.a), 5000);
    try std.testing.expectEqualSlices(u8, &holding, &entry.holding);
    try std.testing.expectEqualStrings(&std.fmt.bytesToHex(holding, .lower), (try f.summary(.a, copy)).?.holding.?);

    // The old Host's resume answers what the Keeper did, and the drop then ends nothing.
    try f.resumed(.b, id, "claimed-elsewhere");
    try f.awaitHandoverState(.b, id, "moved", 1000);
    try expectKind(try f.dropOn(.b, id), MessageKind.drop_terminal);
    try f.awaitGone(.b, id, 1000);
    try std.testing.expect(f.running(copy));
    try f.expectCounts(.{ .b = before.b }, 5000);

    try expectReadOn(f, copy);
    try std.testing.expectEqual(@as(usize, 0), f.eventsNaming(.b, MessageKind.terminal_exited, id));
    try f.dispose(.a, copy);
    f.unwatch(copy);
    try f.expectCounts(before.all(), 10_000);
}

test "a move away from this Host drops its copy and returns every handle" {
    var f = try Fixture.create();
    defer f.destroy();
    const before = try f.counts();
    const id = try f.createTerminal(.a);
    const copy = try f.moveUntilDrop(.a, .b, id);
    try expectKind(try f.dropOn(.a, id), MessageKind.drop_terminal);
    try f.awaitGone(.a, id, 1000);
    try std.testing.expect(f.running(copy));
    try std.testing.expectEqual(@as(usize, 0), f.eventsNaming(.a, MessageKind.terminal_exited, id));
    try f.expectCounts(.{ .a = before.a }, 5000);

    // Ending the terminal on its owner ends the program and OpenConsole.
    try endOn(f, .b, &.{copy});
    _ = try f.awaitExitCode(.b, copy, 10_000);
    try f.dispose(.b, copy);
    f.unwatch(copy);
    try f.expectCounts(before.all(), 10_000);
}

test "a mark the Keeper refuses leaves the terminal reading on its old Host with nothing lost" {
    var f = try Fixture.create();
    defer f.destroy();
    const before = try f.counts();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    // The Keeper is no registered Host, so it refuses a mark naming it, and nothing applied.
    try expectFailure(try f.quiesce(.a, id, f.keeper.?.identity, 30), "move-failed", "not-registered");
    try f.awaitHandoverState(.a, id, null, 1000);
    try f.expectCounts(held.all(), 5000);
    try expectReadOn(f, id);
    try f.dispose(.a, id);
    f.unwatch(id);
    try f.expectCounts(before.all(), 10_000);
}

test "a mark left unanswered stops the terminal until the Worker ends the Keeper and resumes it" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    try f.freezeKeeper();
    try expectFailure(try f.quiesce(.a, id, f.identity(.b), 30), "move-failed", "unanswered");
    _ = try f.awaitEvent(.a, MessageKind.keeper_unresponsive, 1, 5000);
    // Possibly marked: the terminal stays stopped.
    try f.awaitHandoverState(.a, id, "quiesced", 1000);
    try f.replaceKeeper();
    try f.resumed(.a, id, "resumed");
    try f.awaitHandoverState(.a, id, null, 1000);
    // Read on, it is held again, by the next Keeper.
    _ = try f.awaitHolding(id, f.identity(.a), 5000);
    try f.expectCounts(.{ .a = held.a, .b = held.b }, 5000);
    try expectReadOn(f, id);
}

test "a record of an earlier quiesce is refused as stale-mark and the terminal reads on" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    const earlier = try f.readRecord(.a, id, try f.quiesced(.a, id, .b, 30));
    try f.resumed(.a, id, "resumed");
    _ = try f.quiesced(.a, id, .b, 30);
    const creation = f.creationId();
    try f.importRecord(.b, creation, earlier, 30);
    try expectFailure(try f.restore(.b, creation), "move-failed", "stale-mark");
    try std.testing.expectEqual(@as(usize, 0), (try f.inventory(.b)).len);
    try f.expectCounts(.{ .b = held.b, .keeper = held.keeper }, 5000);
    try f.resumed(.a, id, "resumed");
    try f.expectCounts(held.all(), 5000);
    try expectReadOn(f, id);
}

test "a claim the Keeper refuses never lets the new Host read" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    const copy = try f.restored(.a, .b, id, try f.quiesced(.a, id, .b, 30), 30);
    try f.awaitHandoverState(.b, copy, "restored", 1000);
    // The old Host resumes through the Keeper first, which ends the mark the claim names.
    try f.resumed(.a, id, "resumed");
    try expectFailure(try f.claim(.b, copy), "move-failed", "stale-mark");
    try f.awaitGone(.b, copy, 1000);
    try f.expectCounts(held.all(), 5000);
    try expectReadOn(f, id);
}

test "a claim left unanswered never lets the new Host read and ends with one reader once the Keeper is ended" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    const copy = try f.restored(.a, .b, id, try f.quiesced(.a, id, .b, 30), 30);
    try f.freezeKeeper();
    try expectFailure(try f.claim(.b, copy), "move-failed", "unanswered");
    try f.awaitHandoverState(.b, copy, "restored", 1000);
    try f.awaitHandoverState(.a, id, "quiesced", 1000);
    // Neither Host reads. Ending the Keeper undoes whatever the claim did: the new Host
    // discards its import, and only then does the old Host resume without the Keeper.
    try f.replaceKeeper();
    try expectDiscarded(f, try f.discard(.b, try f.targetPayload(copy)));
    try f.awaitGone(.b, copy, 1000);
    try f.resumed(.a, id, "resumed");
    _ = try f.awaitHolding(id, f.identity(.a), 5000);
    try f.expectCounts(.{ .a = held.a, .b = held.b }, 5000);
    try expectReadOn(f, id);
}

test "a resume left unanswered keeps the terminal stopped until the Worker resumes it without the Keeper" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    _ = try f.quiesced(.a, id, .b, 30);
    try f.freezeKeeper();
    try expectFailure(try f.resumeOn(.a, id), "move-failed", "unanswered");
    try f.awaitHandoverState(.a, id, "quiesced", 1000);
    try f.replaceKeeper();
    try f.resumed(.a, id, "resumed");
    _ = try f.awaitHolding(id, f.identity(.a), 5000);
    try f.expectCounts(.{ .a = held.a, .b = held.b }, 5000);
    try expectReadOn(f, id);
}

test "a quiesce held inside a sequence is refused as mid-sequence and the terminal reads on" {
    var f = try Fixture.create();
    defer f.destroy();
    // Once it has shown `lines-end`, the probe writes nothing until a key arrives, while the witness
    // leaves Host A's parser inside a sequence; the black-box run refuses a program stopped inside
    // its own.
    const id = try f.createTerminal(.a);
    const printed = try support.titleNumber(try f.typeAndAwait(.a, id, "x", "lines-end ", true));
    _ = try f.awaitLine(id, try std.fmt.allocPrint(f.arena(), "lines-end {d}", .{printed}), 10_000);
    const held = try f.counts();
    try f.feed(id, "\x1b[3");
    try expectFailure(try f.quiesce(.a, id, f.identity(.b), 30), "move-passing", "mid-sequence");
    try f.awaitHandoverState(.a, id, null, 1000);
    try f.feed(id, "1m");
    try f.expectCounts(held.all(), 5000);
    // The terminal reads on: the probe shows the next key, below every line it printed.
    _ = try f.typeAndAwait(.a, id, "k", "key-k", false);
    try support.expectNumberedLines(try f.awaitLine(id, "key-k", 10_000), printed);
    try f.typeInput(.a, id, "q");
    try std.testing.expectEqual(@as(?u32, 0), try f.awaitExitCode(.a, id, 10_000));
    try f.awaitEnded(id, 10_000);
}

test "the quiesce deadline resumes the terminal and an unclaimed import discards itself at its own" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();
    const copy = try f.restored(.a, .b, id, try f.quiesced(.a, id, .b, 5), 2);
    try f.awaitHandoverState(.b, copy, "restored", 1000);
    // The import's deadline passes first: the new Host declines and discards it by itself.
    try f.awaitGone(.b, copy, 4000);
    try f.awaitHandoverState(.a, id, "quiesced", 1000);
    // Then the quiesce's: the old Host resumes through the Keeper, and its record is gone.
    try f.awaitHandoverState(.a, id, null, 6000);
    try expectFailure(try f.call(.a, MessageKind.read_handover_record, .control_json, try f.readPayload(id, 0), 5000), "invalid-phase", null);
    try f.expectCounts(held.all(), 5000);

    // An import that never restores ends at its deadline as well.
    const creation = f.creationId();
    try expectKind(try f.call(.b, MessageKind.import_terminal, .control_json, try f.importPayload(creation, 64, 1), 5000), MessageKind.import_terminal);
    try f.idle(1500);
    try expectFailure(try f.call(.b, MessageKind.write_handover_record, .handover_record, try f.chunkPayload(creation, 0, &([_]u8{0} ** 64)), 5000), "missing-resource", null);
    try f.expectCounts(held.all(), 5000);
    try expectReadOn(f, id);
}

test "a discard or the import deadline during the restore's build discards the import when the build ends" {
    var f = try Fixture.create();
    defer f.destroy();
    const id = try f.createTerminal(.a);
    const held = try f.counts();

    // A DiscardImport while the build waits on the Keeper's `inspect` waits for the build.
    var record = try f.readRecord(.a, id, try f.quiesced(.a, id, .b, 30));
    var creation = f.creationId();
    try f.importRecord(.b, creation, record, 30);
    try f.freezeKeeper();
    const restoring = try f.request(.b, MessageKind.restore_terminal, .control_json, try f.creationPayload(creation));
    const discarding = try f.request(.b, MessageKind.discard_import, .control_json, try f.creationPayload(creation));
    try f.idle(800);
    try std.testing.expect(f.answer(.b, discarding) == null);
    try f.thawKeeper();
    const identity = try f.awaitAnswer(.b, restoring, 10_000);
    try expectKind(identity, MessageKind.restore_terminal);
    const copy = try f.copyOf(id, identity.payload);
    try expectDiscarded(f, try f.awaitAnswer(.b, discarding, 10_000));
    try f.awaitGone(.b, copy, 2000);
    try f.expectCounts(.{ .b = held.b }, 5000);

    // The import's deadline passing during the build discards it the same way.
    try f.resumed(.a, id, "resumed");
    record = try f.readRecord(.a, id, try f.quiesced(.a, id, .b, 30));
    creation = f.creationId();
    try f.importRecord(.b, creation, record, 1);
    try f.freezeKeeper();
    const late = try f.request(.b, MessageKind.restore_terminal, .control_json, try f.creationPayload(creation));
    try f.idle(1300);
    try f.thawKeeper();
    const late_identity = try f.awaitAnswer(.b, late, 10_000);
    try expectKind(late_identity, MessageKind.restore_terminal);
    try f.awaitGone(.b, try f.copyOf(id, late_identity.payload), 3000);
    try f.resumed(.a, id, "resumed");
    try f.expectCounts(held.all(), 5000);
    try expectReadOn(f, id);
}

test "a settled terminal moves once its outcome is published and its import sends no TerminalExited" {
    var f = try Fixture.create();
    defer f.destroy();
    const before = try f.counts();
    const id = try f.createTerminal(.a);
    // `x` ends the probe's lines and `q` ends the probe.
    try f.typeInput(.a, id, "xq");
    try std.testing.expectEqual(@as(?u32, 0), try f.awaitExitCode(.a, id, 10_000));
    try f.awaitEnded(id, 10_000);
    const copy = try f.restored(.a, .b, id, try f.quiesced(.a, id, .b, 30), 30);
    const settled = (try f.summary(.b, copy)).?;
    try std.testing.expectEqualStrings("exited", settled.phase);
    try std.testing.expect(settled.handover == null);
    try expectFailure(try f.claim(.b, copy), "move-ineligible", "not-running");
    try f.idle(300);
    try std.testing.expectEqual(@as(usize, 0), f.eventsNaming(.b, MessageKind.terminal_exited, copy));
    try f.dispose(.a, id);
    try f.dispose(.b, copy);
    f.unwatch(id);
    try f.expectCounts(before.all(), 10_000);
}

test "after KeeperLost the next Keeper holds no quiesced, restored, or moved terminal" {
    var f = try Fixture.create();
    defer f.destroy();
    const first = try f.createTerminal(.a);
    const second = try f.createTerminal(.b);
    // `second` moves to A and waits for its drop on B; `first` is quiesced on A and restored on B.
    const second_on_a = try f.moveUntilDrop(.b, .a, second);
    const first_on_b = try f.restored(.a, .b, first, try f.quiesced(.a, first, .b, 30), 30);
    try f.replaceKeeper();
    _ = try f.awaitHolding(second_on_a, f.identity(.a), 5000);
    try f.idle(300);
    const entries = try f.holdings();
    try std.testing.expectEqual(@as(usize, 1), entries.len);
    try std.testing.expectEqualSlices(u8, &second_on_a.bytes, &entries[0].facts.terminal.terminal_id);

    // The Worker settles the rest: the new Host discards, the old Host resumes without the
    // Keeper and is held again, and the moved copy drops.
    try expectDiscarded(f, try f.discard(.b, try f.targetPayload(first_on_b)));
    try f.resumed(.a, first, "resumed");
    _ = try f.awaitHolding(first, f.identity(.a), 5000);
    try expectKind(try f.dropOn(.b, second), MessageKind.drop_terminal);
    try std.testing.expect(f.running(first) and f.running(second_on_a));
    try endOn(f, .a, &.{ first, second_on_a });
}

test "a fatal end of either Host mid-move ends no program the other serves" {
    var f = try Fixture.create();
    defer f.destroy();
    // Host B ends while it holds the restored copy: the old Host resumes through the Keeper.
    const first = try f.createTerminal(.a);
    _ = try f.restored(.a, .b, first, try f.quiesced(.a, first, .b, 30), 30);
    f.killHost(.b);
    try std.testing.expect(f.running(first));
    try f.resumed(.a, first, "resumed");
    try f.awaitHandoverState(.a, first, null, 1000);

    // The next Host B ends while its terminal is quiesced: the restored copy claims it.
    try f.startHost(.b);
    const second = try f.createTerminal(.b);
    const second_on_a = try f.restored(.b, .a, second, try f.quiesced(.b, second, .a, 30), 30);
    f.killHost(.b);
    try std.testing.expect(f.running(second));
    try expectKind(try f.claim(.a, second_on_a), MessageKind.claim_terminal);
    _ = try f.awaitHolding(second_on_a, f.identity(.a), 5000);
    try endOn(f, .a, &.{ first, second_on_a });
}

test "neither Host's exit ends a program the other serves" {
    var f = try Fixture.create();
    defer f.destroy();
    // Host B exits holding a moved copy: its teardown drops it.
    const first = try f.createTerminal(.b);
    const first_on_a = try f.moveUntilDrop(.b, .a, first);
    try std.testing.expectEqual(@as(u32, 0), try f.exitHost(.b));
    try std.testing.expect(f.running(first_on_a));

    // Host A exits holding a restored copy and a moved one; its own terminal ends with it.
    try f.startHost(.b);
    const second = try f.createTerminal(.b);
    _ = try f.restored(.b, .a, second, try f.quiesced(.b, second, .a, 30), 30);
    const third = try f.createTerminal(.a);
    const third_on_b = try f.moveUntilDrop(.a, .b, third);
    try f.endA();
    try f.awaitEnded(first_on_a, 10_000);
    try std.testing.expect(f.running(second) and f.running(third_on_b));
    try f.resumed(.b, second, "resumed");
    try endOn(f, .b, &.{ second, third_on_b });
}
