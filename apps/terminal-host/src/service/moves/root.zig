//! The `handover` capability of the Worker session: the Host's one export and one import, the
//! answers they owe, and their part in the Host's teardown. Only the dispatch thread calls it. The
//! dispatch thread never waits on the Keeper, a parking reader, or an export: each such request
//! completes when its result is published, with its reply reserved up front, and goes out with the
//! session's next events; an answer decided at once goes out with the request's own records.

const std = @import("std");
const builtin = @import("builtin");
const control = @import("terminal-host-control");
const core_module = @import("terminal-host-core").state;
const identifiers = @import("terminal-host-model").identifiers;
const keeper_link = @import("terminal-host-keeper-link");
const platform = @import("terminal-host-platform");
const protocol = @import("terminal-protocol");
const answers_module = @import("answers.zig");
const record_export = @import("export.zig");
const export_teardown = @import("export_teardown.zig");
const record_import = @import("import.zig");
const creation_pipeline = @import("../creation_pipeline.zig");
const host_outgoing = @import("../outgoing.zig");
const host_request_context = @import("../request_context.zig");
const host_runtime_aggregate = @import("../runtime_aggregate/root.zig");

pub const refusal = @import("refusal.zig");
pub const Session = answers_module.Session;

const Env = @import("env.zig").Env;
const Aggregate = host_runtime_aggregate.HostRuntimeAggregate;
const Attachments = keeper_link.Attachments;
const is_windows = builtin.os.tag == .windows;

pub const Moves = struct {
    allocator: std.mem.Allocator,
    answers: answers_module.Answers,
    exporting: ?*record_export.Export = null,
    importing: ?*record_import.Import = null,
    /// Whether the current Worker session negotiated `handover`.
    negotiated: bool = false,
    timer: ?platform.monotonic.Timer,
    stopped: bool = false,

    pub fn init(allocator: std.mem.Allocator) Moves {
        return .{
            .allocator = allocator,
            .answers = .{ .allocator = allocator },
            .timer = platform.monotonic.Timer.start() catch null,
        };
    }

    /// A Worker session was admitted; `negotiated` tells whether both sides listed `handover`.
    pub fn admit(self: *Moves, negotiated: bool) void {
        self.negotiated = negotiated;
    }

    /// The session ended. Its moves go on: an export resumes at its deadline and an import
    /// discards itself at its own, and the answers it was owed are dropped.
    pub fn disconnect(self: *Moves) void {
        self.negotiated = false;
    }

    /// The request context's view, whether or not the session negotiated `handover`, since a
    /// `DetachKeeper` checks the moves either way.
    pub fn access(self: *Moves, attachments: *Attachments) ?Access {
        if (comptime !is_windows) return null;
        if (self.stopped) return null;
        return .{ .moves = self, .attachments = attachments };
    }

    fn env(self: *Moves, aggregate: *Aggregate, creations: *creation_pipeline.Pipeline, attachments: *Attachments) Env {
        return .{
            .allocator = self.allocator,
            .aggregate = aggregate,
            .creations = creations,
            .attachments = attachments,
            .answers = &self.answers,
            .now_ns = if (self.timer) |*timer| timer.read() else 0,
        };
    }

    /// Takes the next step of the export and the import: a worker's or the Keeper's answer, a
    /// restore that settled, or a deadline.
    pub fn advance(self: *Moves, aggregate: *Aggregate, creations: *creation_pipeline.Pipeline, attachments: *Attachments) void {
        if (comptime !is_windows) return;
        if (self.stopped or (self.exporting == null and self.importing == null)) return;
        const current = self.env(aggregate, creations, attachments);
        record_export.advance(current, &self.exporting);
        record_import.advance(current, &self.importing);
    }

    /// Advances, then writes every answer owed to `session`; the answers of an ended session are
    /// dropped, and one that does not fit stays for the next delivery.
    pub fn deliver(
        self: *Moves,
        aggregate: *Aggregate,
        creations: *creation_pipeline.Pipeline,
        attachments: *Attachments,
        session: ?Session,
    ) void {
        if (comptime !is_windows) return;
        if (self.stopped) return;
        self.advance(aggregate, creations, attachments);
        self.answers.deliver(session, aggregate.hostEpoch());
    }

    /// Whether `keeper` holds a mark of this Host's or gave it a restored import, so `DetachKeeper`
    /// waits for the move.
    pub fn holdsKeeper(self: *const Moves, keeper: keeper_link.Keeper) bool {
        if (comptime !is_windows) return false;
        return record_export.holdsMark(&self.exporting, keeper) or record_import.heldBy(&self.importing, keeper);
    }

    /// Host teardown, before the creation pipeline's: a restore build no longer waits for the Keeper.
    pub fn closeMailbox(self: *Moves) void {
        if (comptime !is_windows) return;
        record_import.closeRestore(&self.importing);
    }

    /// Host teardown, after the creation pipeline's and before any terminal's release: a quiesced
    /// terminal ends only after the Keeper's `resumed`, a moved one or a restored import is dropped,
    /// and a claim the Keeper applied completes.
    pub fn teardown(self: *Moves, aggregate: *Aggregate, creations: *creation_pipeline.Pipeline, attachments: *Attachments) void {
        if (comptime !is_windows) return;
        if (self.stopped) return;
        const current = self.env(aggregate, creations, attachments);
        export_teardown.teardown(current, &self.exporting);
        record_import.teardown(current, &self.importing);
    }

    /// Frees the answers, once teardown ended every move. Repeating it changes nothing.
    pub fn stop(self: *Moves) void {
        if (self.stopped) return;
        self.stopped = true;
        std.debug.assert(self.exporting == null and self.importing == null);
        self.answers.deinit();
    }
};

pub const Access = struct {
    moves: *Moves,
    attachments: *Attachments,

    pub fn negotiated(self: Access) bool {
        return self.moves.negotiated;
    }

    /// One `handover` request of a session that negotiated it. What settled since the last turn
    /// comes first, so the request meets it.
    pub fn handle(
        self: Access,
        context: *host_request_context.Context,
        operation: protocol.RequestOperation,
        connection_id: core_module.ConnectionId,
        envelope: protocol.EnvelopeSummary,
        correlation_id: identifiers.IdentifierBytes,
        outgoing: *host_outgoing.Outgoing,
    ) !void {
        if (comptime !is_windows) unreachable;
        const moves = self.moves;
        try moves.answers.reserve();
        const current = moves.env(context.runtime_aggregate, context.creations, self.attachments);
        record_export.advance(current, &moves.exporting);
        record_import.advance(current, &moves.importing);
        const to: answers_module.Pending = .{ .connection_id = connection_id, .correlation_id = correlation_id };
        dispatch(moves, context, current, operation, to, envelope.payload) catch |err| {
            moves.answers.handOver();
            return context.rejectControlDecode(outgoing, correlation_id, err, invalidPayload(operation));
        };
        moves.answers.deliver(.{ .connection_id = connection_id, .outgoing = outgoing }, context.hostEpoch());
    }
};

fn dispatch(
    moves: *Moves,
    context: *host_request_context.Context,
    current: Env,
    operation: protocol.RequestOperation,
    to: answers_module.Pending,
    payload: []const u8,
) control.decode.DecodeError!void {
    const allocator = context.allocator;
    const handover = control.decode.handover;
    switch (operation) {
        .quiesce_terminal => {
            var decoded = try handover.decodeQuiesce(allocator, payload);
            defer decoded.deinit();
            record_export.quiesce(current, &moves.exporting, to, decoded.value);
        },
        .read_handover_record => {
            var decoded = try handover.decodeReadRecord(allocator, payload);
            defer decoded.deinit();
            record_export.read(current, &moves.exporting, to, decoded.value);
        },
        .resume_terminal => {
            var decoded = try control.decode.decodeTerminalTarget(allocator, payload);
            defer decoded.deinit();
            record_export.resumeTerminal(current, &moves.exporting, to, decoded.value);
        },
        .drop_terminal => {
            var decoded = try control.decode.decodeTerminalTarget(allocator, payload);
            defer decoded.deinit();
            record_export.dropTerminal(current, &moves.exporting, to, decoded.value);
        },
        .import_terminal => {
            var decoded = try handover.decodeImport(allocator, payload);
            defer decoded.deinit();
            record_import.importTerminal(current, &moves.importing, to, decoded.value, context.terminal_admission);
        },
        .write_handover_record => record_import.writeRecord(current, &moves.importing, to, try handover.decodeRecordChunk(payload)),
        .restore_terminal => {
            var decoded = try handover.decodeCreation(allocator, payload);
            defer decoded.deinit();
            record_import.restore(current, &moves.importing, to, decoded.value, context.terminal_admission);
        },
        .claim_terminal => {
            var decoded = try control.decode.decodeTerminalTarget(allocator, payload);
            defer decoded.deinit();
            record_import.claimTerminal(current, &moves.importing, to, decoded.value);
        },
        .discard_import => {
            var decoded = try handover.decodeDiscard(allocator, payload);
            defer decoded.deinit();
            record_import.discardImport(current, &moves.importing, to, decoded.value);
        },
        else => unreachable,
    }
}

fn invalidPayload(operation: protocol.RequestOperation) []const u8 {
    return switch (operation) {
        .quiesce_terminal => "QuiesceTerminal payload is invalid",
        .read_handover_record => "ReadHandoverRecord payload is invalid",
        .resume_terminal => "ResumeTerminal payload is invalid",
        .drop_terminal => "DropTerminal payload is invalid",
        .import_terminal => "ImportTerminal payload is invalid",
        .write_handover_record => "WriteHandoverRecord payload is invalid",
        .restore_terminal => "RestoreTerminal payload is invalid",
        .claim_terminal => "ClaimTerminal payload is invalid",
        .discard_import => "DiscardImport payload is invalid",
        else => unreachable,
    };
}
