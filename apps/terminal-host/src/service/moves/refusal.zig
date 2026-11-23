//! The typed failures of the `handover` requests. A refusal names its class, so the Worker can act
//! on it: `move-ineligible` never passes, `move-passing` may pass on a retry, and `move-failed`
//! reports a step that failed, a Keeper's refusal reason included.

const terminal_engine = @import("terminal-host-engine");
const host_response = @import("../response.zig");

pub const Details = host_response.FailureDetails;
const MovePark = terminal_engine.runtime.MovePark;
const Result = @import("terminal-host-keeper-link").moves.Result;

pub fn ineligible(comptime reason: []const u8) Details {
    return .{
        .code = "move-ineligible",
        .message = "The terminal cannot move between Hosts",
        .retryable = false,
        .reason = reason,
    };
}

pub fn passing(comptime reason: []const u8) Details {
    return .{
        .code = "move-passing",
        .message = "The terminal cannot move between Hosts yet",
        .retryable = true,
        .reason = reason,
    };
}

pub fn failed(reason: []const u8) Details {
    return .{
        .code = "move-failed",
        .message = "A step of the move between Hosts failed",
        .retryable = true,
        .reason = reason,
    };
}

pub const moving = passing("moving");

pub const invalid_phase: Details = .{
    .code = "invalid-phase",
    .message = "terminal is not in a phase that accepts this request",
    .retryable = false,
};

pub const missing_import: Details = .{
    .code = "missing-resource",
    .message = "The Host keeps no import of that creation",
    .retryable = false,
};

pub const incomplete_record: Details = .{
    .code = "invalid-phase",
    .message = "The import has not received its whole record",
    .retryable = false,
};

pub const invalid_chunk: Details = .{
    .code = "invalid-field",
    .message = "The record chunk does not continue the import's record",
    .retryable = false,
};

pub const draining: Details = .{
    .code = "host-draining",
    .message = "Terminal Host is draining and accepts no new terminals",
    .retryable = false,
};

pub const exhausted: Details = .{
    .code = "resource-exhausted",
    .message = "Host could not allocate the handover record",
    .retryable = true,
};

/// A park that did not reach ground: the reader reads on.
pub fn park(outcome: MovePark) Details {
    return switch (outcome) {
        .parked => unreachable,
        .input_pending => passing("input-pending"),
        .signal_pending => passing("signal-pending"),
        .mid_sequence => passing("mid-sequence"),
        .stopped => ineligible("not-running"),
    };
}

/// An export that failed: the export is discarded and the reader reads on.
pub fn exported(err: terminal_engine.runtime.HandoverExportError) Details {
    return switch (err) {
        error.HoldsImage => ineligible("holds-image"),
        error.RecordTooLarge => ineligible("too-large"),
        error.Fenced => ineligible("not-running"),
        error.MidSequence, error.NotParked => passing("mid-sequence"),
        error.InputPending => passing("input-pending"),
        error.ResizeInFlight => passing("signal-pending"),
        error.OutOfMemory => exhausted,
        error.Unrepresentable => failed("unrepresentable"),
        error.InvalidRecord => failed("invalid-record"),
    };
}

/// A record the restore refused or could not build.
pub fn restored(err: anyerror) Details {
    return switch (err) {
        error.OutOfMemory => exhausted,
        else => failed("invalid-record"),
    };
}

/// A Keeper request that did not apply. `unanswered` counts as possibly applied. A Keeper's reason
/// names static text, so the failure keeps it.
pub fn keeper(result: Result) Details {
    return switch (result) {
        .refused => |reason| failed(reason.name()),
        .unsupported => failed("unsupported"),
        .unanswered, .keeper_ended => failed("unanswered"),
        .failed => failed("no-keeper"),
        .pull_failed => failed("handle-failed"),
        .done, .outcome, .inspected, .inspected_program => failed("unanswered"),
    };
}
