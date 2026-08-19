const std = @import("std");
const deterministic = @import("deterministic_schedule.zig");

const Seam = enum(u8) {
    creation_preparation,
    creation_spawn,
    creation_admission,
    control_reply_reservation,
    raw_input_reservation,
    semantic_input_reservation,
    input_publication,
    attachment_runtime_preparation,
    attachment_core_commit,
    detach_repair_materialization,
    detach_repair_publication,
    terminal_control_mutation,
    resize_effect,
    resize_rollback,
    projection_capture_encoding,
    projection_payload_adoption,
    projection_read_commit,
    interaction_provisional_mutation,
    batch_prefix_full,
    lifecycle_outcome_handoff,
    local_ipc_partial_write,
    process_observation_containment,
    multi_runtime_shutdown,
    deferred_start_spawn,
    runtime_theme_commit,
};

const Disposition = enum {
    contain_and_retain,
    preserve_for_retry,
    publish_prefix,
    reject_without_commit,
    rollback,
};

const Schedule = enum {
    allocation_index,
    deterministic_interleaving,
    native_boundary,
    single_owner,
};

const Evidence = struct {
    source: []const u8,
    test_name: []const u8,
};

const Entry = struct {
    disposition: Disposition,
    evidence: Evidence,
    id: []const u8,
    name: []const u8,
    schedule: Schedule,
    seam: Seam,
};

const matrix = [_]Entry{
    entry(.creation_preparation, "CS-001", "CreationPreparation", .rollback, .allocation_index, "src/engine/__tests__/runtime_state_test.zig", "owned terminal profile releases every partial allocation"),
    entry(.creation_spawn, "CS-002", "CreationSpawn", .contain_and_retain, .native_boundary, "src/service/__tests__/runtime_aggregate_lifecycle_test.zig", "failed runtime creation leaves no retained terminal commit"),
    entry(.creation_admission, "CS-003", "CreationAdmission", .rollback, .allocation_index, "src/service/__tests__/terminal_integration_test.zig", "two PTY runtimes route input independently and isolate spawn failure"),
    entry(.control_reply_reservation, "CS-004", "ControlReplyReservation", .reject_without_commit, .allocation_index, "src/service/__tests__/connection_request_test.zig", "reply reservation failure cannot commit terminal defaults"),
    entry(.raw_input_reservation, "CS-005", "RawInputReservation", .reject_without_commit, .single_owner, "src/engine/__tests__/runtime_input_test.zig", "runtime queue applies bounded backpressure before allocation"),
    entry(.semantic_input_reservation, "CS-006", "SemanticInputReservation", .rollback, .allocation_index, "src/engine/__tests__/runtime_input_test.zig", "zero-byte semantic encodings do not retain queue metadata"),
    entry(.input_publication, "CS-007", "InputPublication", .rollback, .deterministic_interleaving, "src/engine/__tests__/runtime_input_test.zig", "client input publication linearizes against post-exit client closure"),
    entry(.attachment_runtime_preparation, "CS-008", "AttachmentRuntimePreparation", .rollback, .single_owner, "src/service/__tests__/runtime_aggregate_attachment_test.zig", "read attachment is not committed when its terminal runtime is missing"),
    entry(.attachment_core_commit, "CS-009", "AttachmentCoreCommit", .rollback, .allocation_index, "src/core/__tests__/registry_broker_test.zig", "first attachment allocation failures leave no broker commit"),
    entry(.detach_repair_materialization, "CS-010", "DetachRepairMaterialization", .contain_and_retain, .allocation_index, "src/engine/__tests__/runtime_input_test.zig", "attachment detach repair allocation failure fences the live runtime"),
    entry(.detach_repair_publication, "CS-011", "DetachRepairPublication", .preserve_for_retry, .deterministic_interleaving, "src/engine/__tests__/runtime_input_test.zig", "prepared attachment input cannot commit after its owner detaches"),
    entry(.terminal_control_mutation, "CS-012", "TerminalControlMutation", .reject_without_commit, .single_owner, "src/engine/__tests__/runtime_state_test.zig", "application terminal controls reject counter exhaustion before mutation"),
    entry(.resize_effect, "CS-013", "ResizeEffect", .contain_and_retain, .native_boundary, "src/engine/__tests__/runtime_state_test.zig", "terminal-driven PTY resize failure fences before Ghostty mutation"),
    entry(.resize_rollback, "CS-014", "ResizeRollback", .contain_and_retain, .deterministic_interleaving, "src/engine/__tests__/runtime_state_test.zig", "platform resize errors restore committed geometry or report an unsafe rollback"),
    entry(.projection_capture_encoding, "CS-015", "ProjectionCaptureEncoding", .preserve_for_retry, .allocation_index, "src/engine/__tests__/runtime_state_test.zig", "runtime resets projection resources after encode failure"),
    entry(.projection_payload_adoption, "CS-016", "ProjectionPayloadAdoption", .rollback, .allocation_index, "src/service/__tests__/outgoing_test.zig", "Outgoing adoption allocation failure preserves caller ownership and reservations"),
    entry(.projection_read_commit, "CS-017", "ProjectionReadCommit", .preserve_for_retry, .deterministic_interleaving, "src/service/__tests__/projection_test.zig", "THCR-018 winning read commit precedes a waiting failure publisher"),
    entry(.interaction_provisional_mutation, "CS-018", "InteractionProvisionalMutation", .rollback, .allocation_index, "src/service/__tests__/interaction_transaction_test.zig", "allocator failure rolls back every earlier interaction mutation"),
    entry(.batch_prefix_full, "CS-019", "BatchPrefixFull", .publish_prefix, .single_owner, "src/service/__tests__/projection_test.zig", "a full dispatch batch preserves its committed projection prefix"),
    entry(.lifecycle_outcome_handoff, "CS-020", "LifecycleOutcomeHandoff", .preserve_for_retry, .single_owner, "src/service/__tests__/event_delivery_test.zig", "full outgoing batch leaves a failed outcome pending for the next handoff"),
    entry(.local_ipc_partial_write, "CS-021", "LocalIpcPartialWrite", .preserve_for_retry, .native_boundary, "src/transport/__tests__/local_endpoint_test.zig", "multi-record handoff stops at the first write failure"),
    entry(.process_observation_containment, "CS-022", "ProcessObservationContainment", .contain_and_retain, .native_boundary, "src/service/__tests__/runtime_aggregate_lifecycle_test.zig", "process observation failure maps to a distinct runtime failure"),
    entry(.multi_runtime_shutdown, "CS-023", "MultiRuntimeShutdown", .contain_and_retain, .deterministic_interleaving, "src/service/__tests__/runtime_aggregate_cleanup_test.zig", "runtime destruction starts only after every shutdown succeeds"),
    entry(.deferred_start_spawn, "CS-024", "DeferredStartSpawn", .contain_and_retain, .native_boundary, "src/service/__tests__/runtime_aggregate_lifecycle_test.zig", "deferred creation retains launch ownership and failed start stays retryable"),
    entry(.runtime_theme_commit, "CS-025", "RuntimeThemeCommit", .reject_without_commit, .single_owner, "src/service/__tests__/runtime_aggregate_theme_test.zig", "runtime theme failure never commits the semantic theme revision"),
};

test "commit seam matrix is exhaustive unique documented and backed by named tests" {
    try std.testing.expectEqual(std.meta.fields(Seam).len, matrix.len);
    const runtime_model = try std.fs.cwd().readFileAlloc(
        std.testing.allocator,
        "docs/runtime-model.md",
        2 * 1024 * 1024,
    );
    defer std.testing.allocator.free(runtime_model);

    for (matrix, 0..) |item, index| {
        try std.testing.expectEqual(index, @as(usize, @intFromEnum(item.seam)));
        try std.testing.expect(std.mem.indexOf(u8, runtime_model, item.id) != null);
        try std.testing.expect(std.mem.indexOf(u8, runtime_model, item.name) != null);
        for (matrix[index + 1 ..]) |candidate| {
            try std.testing.expect(!std.mem.eql(u8, item.id, candidate.id));
            try std.testing.expect(!std.mem.eql(u8, item.name, candidate.name));
        }
        try expectNamedTest(item.evidence);
    }
}

test "scripted scheduler admits one declared interleaving" {
    const Event = enum { worker_ready, main_release, worker_commit };
    const ScheduleScript = deterministic.Script(Event);
    const expected = [_]Event{ .worker_ready, .main_release, .worker_commit };
    var script = ScheduleScript.init(&expected);
    const Worker = struct {
        fn run(schedule: *ScheduleScript) void {
            schedule.arrive(.worker_ready);
            schedule.arrive(.worker_commit);
        }
    };
    const worker = try std.Thread.spawn(.{}, Worker.run, .{&script});
    script.waitForCursor(1);
    script.arrive(.main_release);
    worker.join();
    try script.expectComplete();

    const rejected = [_]Event{.worker_ready};
    var mismatched = ScheduleScript.init(&rejected);
    mismatched.arrive(.worker_commit);
    try std.testing.expectError(error.ScheduleMismatch, mismatched.expectComplete());
}

fn entry(
    seam: Seam,
    id: []const u8,
    name: []const u8,
    disposition: Disposition,
    schedule: Schedule,
    source: []const u8,
    test_name: []const u8,
) Entry {
    return .{
        .disposition = disposition,
        .evidence = .{ .source = source, .test_name = test_name },
        .id = id,
        .name = name,
        .schedule = schedule,
        .seam = seam,
    };
}

fn expectNamedTest(evidence: Evidence) !void {
    const source = try std.fs.cwd().readFileAlloc(
        std.testing.allocator,
        evidence.source,
        2 * 1024 * 1024,
    );
    defer std.testing.allocator.free(source);
    const declaration = try std.fmt.allocPrint(
        std.testing.allocator,
        "test \"{s}\"",
        .{evidence.test_name},
    );
    defer std.testing.allocator.free(declaration);
    try std.testing.expect(std.mem.indexOf(u8, source, declaration) != null);
}
