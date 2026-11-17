//! Shared steps of the handover tests: a terminal fed by scripted reads, its export once its
//! reader parks, an import whose reads are scripted too, and the comparison of the two.
const harness = @import("runtime_harness.zig");
const terminal_equality = @import("terminal_equality.zig");
const ScriptedOutput = @import("scripted_output.zig").ScriptedOutput;
const std = harness.std;
const builtin = harness.target_builtin;
const pty_process = harness.pty_process;
const subject = harness.subject;
const TerminalRuntime = harness.TerminalRuntime;
const ReaderParkWait = subject.ReaderParkWait;

pub const handover = harness.engine_boundary.handover;
pub const allocator = std.testing.allocator;
pub const park_timeout_ns = 10 * std.time.ns_per_s;

/// The facts the exporting Host's owner supplies.
pub const inputs: subject.HandoverInputs = .{
    .terminal = .{
        .identity = .{
            .creation_id = [_]u8{0x11} ** 16,
            .host_epoch = [_]u8{0x12} ** 16,
            .terminal_id = [_]u8{0x13} ** 16,
        },
        .lifecycle = .{ .running = .none },
        .start_geometry = null,
        .launch_fingerprint = [_]u8{0x14} ** 32,
        .environment_overlay_digest = [_]u8{0x15} ** 32,
    },
    .configuration_revision = 3,
    .theme = .{
        .operation_id = .fromBytes([_]u8{0x16} ** 16),
        .revision = 2,
        .terminal = harness.model_boundary.theme.default,
    },
};

/// The process a restored runtime starts until the Host adopts a held console: one that reads
/// its input and writes nothing unasked.
pub const StandIn = struct {
    executable: []const u8,
    owned: bool,

    pub fn init() !StandIn {
        if (builtin.os.tag != .windows) return .{ .executable = "/bin/cat", .owned = false };
        return .{ .executable = try pty_process.environmentVariableOwned(allocator, "ComSpec"), .owned = true };
    }

    pub fn deinit(self: *StandIn) void {
        if (self.owned) allocator.free(self.executable);
        self.* = undefined;
    }

    pub fn launch(self: *const StandIn, columns: u16, rows: u16) pty_process.Launch {
        const windows_arguments = [_][]const u8{ "/D", "/S", "/C", "more" };
        return .{
            .arguments = if (builtin.os.tag == .windows) &windows_arguments else &.{},
            .columns = columns,
            .current_directory = null,
            .environment = &.{},
            .executable = self.executable,
            .rows = rows,
        };
    }
};

/// Fails the first worker a restore starts, so an accepted record stops just before anything runs.
pub const RefusingSpawner = struct {
    spawns: usize = 0,

    pub fn spawn(self: *RefusingSpawner, comptime thread_function: anytype, runtime: anytype) !std.Thread {
        _ = thread_function;
        _ = runtime;
        self.spawns += 1;
        return error.InjectedSpawnFailure;
    }
};

pub fn createScripted(output: *ScriptedOutput, stand_in: *const StandIn, columns: u16, rows: u16) !*TerminalRuntime {
    var spawner: harness.HookedThreadSpawner = .{ .hooks = output.hooks() };
    const runtime = try harness.createWithOptionsUsingSpawner(allocator, stand_in.launch(columns, rows), .{}, &spawner);
    output.runtime = runtime;
    return runtime;
}

pub fn park(runtime: *TerminalRuntime) !void {
    try runtime.pauseReader();
    try std.testing.expectEqual(ReaderParkWait.parked, runtime.awaitReaderParked(park_timeout_ns));
}

/// Exports a parked terminal and resumes its reader.
pub fn exportParked(runtime: *TerminalRuntime) ![]u8 {
    try park(runtime);
    const result = runtime.exportHandover(allocator, inputs);
    runtime.resumeReader() catch |err| {
        if (result) |record| allocator.free(record) else |_| {}
        return err;
    };
    return result;
}

pub const Imported = struct {
    runtime: *TerminalRuntime,
    facts: subject.HandoverFacts,

    pub fn deinit(self: *Imported) void {
        allocator.free(self.facts.held_attachments);
        self.runtime.destroy();
        self.* = undefined;
    }
};

/// Imports `record` into a runtime whose reads `output` scripts.
pub fn importScripted(record: []const u8, output: *ScriptedOutput, stand_in: *const StandIn) !Imported {
    var spawner: harness.HookedThreadSpawner = .{ .hooks = output.hooks() };
    const imported = try subject.testing.importHandoverUsingSpawner(allocator, record, stand_in.launch(80, 24), &spawner);
    output.runtime = imported.runtime;
    return .{ .runtime = imported.runtime, .facts = imported.facts };
}

pub fn expectSameTerminal(expected: *TerminalRuntime, actual: *TerminalRuntime) !void {
    const expected_state = harness.runtimeState(expected);
    const actual_state = harness.runtimeState(actual);
    expected_state.terminal_mutex.lock();
    defer expected_state.terminal_mutex.unlock();
    actual_state.terminal_mutex.lock();
    defer actual_state.terminal_mutex.unlock();
    try terminal_equality.expectEqualTerminals(&expected_state.terminal, &actual_state.terminal);
}

/// Compares every section but the screen, which `expectSameTerminal` compares, by exporting both
/// terminals. Ages and remaining durations travel as durations and leave out the time a record
/// spends between Hosts, so here only their presence must agree; `expectDurationsCarried` checks
/// their values.
pub fn expectSameSections(expected: *TerminalRuntime, actual: *TerminalRuntime) !void {
    const expected_record = try exportParked(expected);
    defer allocator.free(expected_record);
    const actual_record = try exportParked(actual);
    defer allocator.free(actual_record);
    var expected_decoded = try handover.decode(allocator, expected_record, subject.handover_pin);
    defer expected_decoded.deinit();
    var actual_decoded = try handover.decode(allocator, actual_record, subject.handover_pin);
    defer actual_decoded.deinit();
    try std.testing.expectEqualStrings(expected_decoded.product_version, actual_decoded.product_version);
    try std.testing.expectEqualDeep(expected_decoded.terminal, actual_decoded.terminal);
    try std.testing.expectEqualDeep(expected_decoded.configuration, actual_decoded.configuration);
    try std.testing.expectEqualDeep(expected_decoded.console, actual_decoded.console);
    var expected_engine = expected_decoded.engine;
    var actual_engine = actual_decoded.engine;
    try std.testing.expectEqual(expected_engine.prompt_gate.anchor == null, actual_engine.prompt_gate.anchor == null);
    for (durations(&expected_engine), durations(&actual_engine)) |expected_slot, actual_slot| {
        const expected_value = expected_slot orelse continue;
        const actual_value = actual_slot.?;
        try std.testing.expectEqual(expected_value.* == null, actual_value.* == null);
        expected_value.* = null;
        actual_value.* = null;
    }
    try std.testing.expectEqualDeep(expected_engine, actual_engine);
}

/// An imported terminal's ages go on from the record's and its remaining durations count down
/// from the record's.
pub fn expectDurationsCarried(record: []const u8, imported: *TerminalRuntime) !void {
    const exported = try exportParked(imported);
    defer allocator.free(exported);
    var carried = try handover.decode(allocator, record, subject.handover_pin);
    defer carried.deinit();
    var now = try handover.decode(allocator, exported, subject.handover_pin);
    defer now.deinit();
    const carried_values = durations(&carried.engine);
    const now_values = durations(&now.engine);
    for (carried_values, now_values, 0..) |before_slot, after_slot, index| {
        try std.testing.expectEqual(before_slot == null, after_slot == null);
        const before = before_slot orelse continue;
        const after = after_slot.?;
        try std.testing.expectEqual(before.* == null, after.* == null);
        const value = before.* orelse continue;
        const age = index == 0 or index == 3;
        try std.testing.expect(if (age) after.*.? >= value else after.*.? <= value);
    }
}

/// The command's elapsed time, the two remaining deadlines, and the prompt anchor's age when the
/// gate holds an anchor.
fn durations(engine: *handover.EngineState) [4]?*?u64 {
    return .{
        &engine.command_elapsed_ns,
        &engine.synchronized_output_remaining_ns,
        &engine.cursor_paint_remaining_ns,
        if (engine.prompt_gate.anchor) |*anchor| &anchor.marked_age_ns else null,
    };
}
