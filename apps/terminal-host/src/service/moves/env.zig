//! What every step of a move reads: the dispatch thread's authority over terminals, the creation
//! pipeline, and the Keeper attachments, the answers that wait for their session, and the time.

const std = @import("std");
const keeper_link = @import("terminal-host-keeper-link");
const answers_module = @import("answers.zig");
const creation_pipeline = @import("../creation_pipeline.zig");
const host_runtime_aggregate = @import("../runtime_aggregate/root.zig");

pub const Env = struct {
    allocator: std.mem.Allocator,
    aggregate: *host_runtime_aggregate.HostRuntimeAggregate,
    creations: *creation_pipeline.Pipeline,
    attachments: *keeper_link.Attachments,
    answers: *answers_module.Answers,
    /// The moves owner's monotonic time.
    now_ns: u64,
};
