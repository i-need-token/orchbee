//! Host teardown's end of an export, once the dispatch loop has ended, before any terminal's
//! release: what is in flight settles as the loop would have settled it; then a quiesced terminal
//! ends only after the Keeper's `resumed`, and a moved one, or one whose resume got any other answer
//! or none, is dropped, so its program runs on in the Keeper's care.

const record_export = @import("export.zig");

const Env = @import("env.zig").Env;
const Export = record_export.Export;

pub fn teardown(env: Env, slot: *?*Export) void {
    while (slot.*) |self| {
        if (self.worker != null) {
            // The park and the encode are bounded, so the join waits at most for their deadlines.
            // No mark was sent, so the export is discarded and the reader reads on.
            switch (record_export.joinWorker(self)) {
                .record => |bytes| env.allocator.free(bytes),
                .refused => {},
            }
            record_export.rollback(env, self);
            return record_export.finish(env, slot);
        } else if (self.requesting) {
            const result = self.request.wait();
            self.requesting = false;
            record_export.settleRequest(env, slot, result);
        } else break;
    }
    const self = slot.* orelse return;
    self.quiesce = null;
    self.asked = null;
    switch (self.phase) {
        .settled => record_export.finish(env, slot),
        .marked, .stopped => resumeOrDrop(env, slot),
        .moved, .dropping => drop(env, slot),
        .parking, .marking, .resuming => unreachable,
    }
}

fn resumeOrDrop(env: Env, slot: *?*Export) void {
    const self = slot.*.?;
    if (record_export.keeperAnswers(env, self)) {
        self.request = .init(.{ .@"resume" = .{ .gate = self.gate(), .marked = .{ .holding = self.holding, .mark = self.mark } } });
        env.attachments.submit(self.keeper.?, &self.request);
        const result = self.request.wait();
        if (result == .outcome and result.outcome == .resumed) {
            record_export.rollback(env, self);
            return record_export.finish(env, slot);
        }
    }
    drop(env, slot);
}

/// Drops the terminal once no link touches its gate: a held slot is forgotten without a release,
/// and a link on its way out settles its slots.
fn drop(env: Env, slot: *?*Export) void {
    const self = slot.*.?;
    const gate = self.gate();
    if (gate.heldIndex() != null) {
        self.request = .init(.{ .forget = gate });
        env.attachments.submit(self.keeper.?, &self.request);
        _ = self.request.wait();
    }
    gate.awaitIdle();
    env.aggregate.removeMovingAtTeardown(self.terminal_id);
    record_export.finish(env, slot);
}
