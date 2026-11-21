//! The screen gate: the rules one screen-gated key passes, at preparation and
//! again at publication, before the Host types it. The sender decided on a
//! frame it read earlier; the gate holds only while that frame is still the
//! screen and no person has acted on this terminal since the sender first saw
//! what it answers, nor shortly before the key.
const std = @import("std");
const protocol = @import("terminal-protocol");

/// A person's input this recent refuses the key. The sender cannot shorten it.
pub const quiet_nanoseconds: u64 = protocol.Limits.screen_gate_quiet_milliseconds * std.time.ns_per_ms;

pub const Error = error{
    ScreenChanged,
    ScreenAlreadyTyped,
    ScreenInputPending,
    ScreenPersonInput,
    ScreenPersonRecent,
};

/// The frame the sender decided on and the revision at which it first saw
/// what it answers.
pub const Gate = struct {
    generation: u64,
    revision: u64,
    first_revision: u64,
};

/// The content revision and the time at which a person last acted.
pub const PersonInput = struct {
    revision: u64,
    /// Null when the runtime had no clock; such an input never grows old.
    at_nanoseconds: ?u64,
};

/// What the runtime remembers about client input. Every client input is a
/// person's, whether or not it encodes to bytes, except a focus report,
/// pointer motion without a button, and a screen-gated key. input_mutex
/// protects this.
pub const InputRecord = struct {
    /// A runtime that does not know its history, such as one built from a
    /// handover record, starts as if a person had acted on every screen.
    person: ?PersonInput = .{ .revision = std.math.maxInt(u64), .at_nanoseconds = null },
    /// The revision the last screen-gated key was typed on.
    typed_revision: ?u64 = null,

    /// A terminal this Host launched: no person has acted on it yet.
    pub const launched: InputRecord = .{ .person = null };

    pub fn notePerson(self: *InputRecord, revision: u64, now_nanoseconds: ?u64) void {
        self.person = .{ .revision = revision, .at_nanoseconds = now_nanoseconds };
    }

    pub fn noteTyped(self: *InputRecord, revision: u64) void {
        self.typed_revision = revision;
    }
};

/// The terminal facts one check reads, sampled under terminal_mutex then
/// input_mutex.
pub const Screen = struct {
    generation: u64,
    revision: u64,
    /// A reader batch is between its parse and its commit.
    committing: bool,
    /// Client entries reserved or queued, the checked key's own included.
    pending_entries: usize,
    now_nanoseconds: ?u64,
};

/// Returns the first rule that fails. `own_entries` is what the checked key
/// itself holds in the queue: nothing at preparation, one entry at publication.
pub fn check(gate: Gate, screen: Screen, record: InputRecord, own_entries: usize) Error!void {
    if (screen.committing or screen.generation != gate.generation or
        screen.revision != gate.revision) return error.ScreenChanged;
    if (record.typed_revision) |typed| {
        if (typed == gate.revision) return error.ScreenAlreadyTyped;
    }
    if (screen.pending_entries != own_entries) return error.ScreenInputPending;
    const person = record.person orelse return;
    if (person.revision >= gate.first_revision) return error.ScreenPersonInput;
    const at = person.at_nanoseconds orelse return error.ScreenPersonRecent;
    const now = screen.now_nanoseconds orelse return error.ScreenPersonRecent;
    if (now -| at < quiet_nanoseconds) return error.ScreenPersonRecent;
}
