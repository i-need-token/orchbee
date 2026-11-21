//! Release first, as the platform enforces it. Each Windows terminal carries one gate in front of
//! the end of its handles: before its pseudoconsole, signal, reference, or Job handle closes, or its
//! Job is terminated, the gate has passed, so every Keeper link that queued or holds the terminal
//! has released it or given up after its reply deadline. A link reports each step here, and only
//! the link thread waits on a Keeper.
//!
//! The dispatch thread never waits here: `tryPass` starts the release and reports whether it is
//! done, and an end that needs the gate waits for the next try. The last settle wakes the service
//! loop for that try. `pass` waits for the links, and only the wait thread and the Host's teardown,
//! after its dispatch loop has ended, call it.
//!
//! A slot belongs to one attachment, named by its serial. While a slot is active the terminal's
//! handles stay open, so its link may read them; the first try wakes each active slot's link
//! through the event the link registered, and the gate passes once no slot is active. A link never
//! touches a gate whose slot it has settled, since the terminal may then end at once.

const std = @import("std");
const builtin = @import("builtin");
const output_wake = @import("output_wake.zig");
const thread_sync = @import("thread_sync.zig");

/// A Host holds at most this many Keeper attachments.
pub const maximum_attachments = 2;

pub const Gate = if (builtin.os.tag != .windows) void else struct {
    const windows = std.os.windows;

    mutex: thread_sync.Mutex = .{},
    condition: thread_sync.Condition = .{},
    /// Set by the first try: the terminal is ending, so no hold starts and every held slot
    /// releases.
    ending: bool = false,
    /// A random id minted at the terminal's first hold and kept for its life, so every Keeper
    /// holds the terminal under the same id.
    holding_id: ?[16]u8 = null,
    slots: [maximum_attachments]Slot = @splat(.{}),

    pub const State = enum {
        idle,
        /// The hold waits for its link thread.
        queued,
        /// The link reads the handles or waits for the hold's reply.
        holding,
        held,
        releasing,
    };

    const Slot = struct {
        /// The attachment the slot last served; zero before any.
        serial: u64 = 0,
        state: State = .idle,
        /// The link's wake event, registered while the slot is active.
        wake: ?windows.HANDLE = null,

        fn active(self: Slot) bool {
            return self.state != .idle;
        }
    };

    extern "kernel32" fn SetEvent(event: windows.HANDLE) callconv(.winapi) windows.BOOL;

    /// Queues this terminal's hold for the attachment `serial` in slot `index` and returns its
    /// holding id. Null once the terminal is ending or when this attachment already tried it.
    pub fn queue(self: *Gate, index: usize, serial: u64, wake: windows.HANDLE) ?[16]u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        const slot = &self.slots[index];
        if (self.ending or slot.serial == serial) return null;
        // An earlier attachment's link settles every slot before it ends.
        std.debug.assert(!slot.active());
        slot.* = .{ .serial = serial, .state = .queued, .wake = wake };
        if (self.holding_id == null) {
            var id: [16]u8 = undefined;
            thread_sync.io().random(&id);
            self.holding_id = id;
        }
        return self.holding_id.?;
    }

    /// The link takes a queued hold. False when the terminal began ending first, which cancels it.
    pub fn take(self: *Gate, index: usize) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(self.slots[index].state == .queued);
        if (self.ending) {
            self.settleLocked(index);
            return false;
        }
        self.slots[index].state = .holding;
        return true;
    }

    /// Cancels a queued hold once the terminal began ending, so the gate does not wait for a hold
    /// the link has not reached. True when it cancelled.
    pub fn cancelIfEnding(self: *Gate, index: usize) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (!self.ending or self.slots[index].state != .queued) return false;
        self.settleLocked(index);
        return true;
    }

    /// Records a hold the Keeper answered. True when the slot must release now, because the
    /// terminal began ending or `release_now` asks for it.
    pub fn held(self: *Gate, index: usize, release_now: bool) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(self.slots[index].state == .holding);
        if (self.ending or release_now) {
            self.slots[index].state = .releasing;
            return true;
        }
        self.slots[index].state = .held;
        return false;
    }

    /// Starts the release of a held slot: always when `detaching`, otherwise once the terminal is
    /// ending. False when the slot does not hold or need not release yet.
    pub fn startRelease(self: *Gate, index: usize, detaching: bool) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        if (self.slots[index].state != .held or !(detaching or self.ending)) return false;
        self.slots[index].state = .releasing;
        return true;
    }

    /// Ends the slot's activity once nothing is held there, or the hold failed, or its Keeper
    /// missed a deadline or ended. The link must not touch this gate afterwards.
    pub fn settle(self: *Gate, index: usize) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.settleLocked(index);
    }

    fn settleLocked(self: *Gate, index: usize) void {
        self.slots[index].state = .idle;
        self.slots[index].wake = null;
        self.condition.broadcast();
        // The gate passed: the service loop retries the end that waited for it.
        if (self.ending and !self.anyActive()) output_wake.signal();
    }

    /// Release first, without waiting: from now on no hold starts and every held slot releases.
    /// True once no slot is active. Each active slot's link bounds its release by the Keeper's
    /// reply deadline, and its last settle wakes the service loop.
    pub fn tryPass(self: *Gate) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.endLocked();
        return !self.anyActive();
    }

    /// Release first, waiting for the links: for the wait thread and the Host's teardown alone.
    pub fn pass(self: *Gate) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        self.endLocked();
        while (self.anyActive()) self.condition.wait(&self.mutex);
    }

    fn endLocked(self: *Gate) void {
        if (self.ending) return;
        self.ending = true;
        for (self.slots) |slot| {
            if (slot.active()) if (slot.wake) |wake| {
                _ = SetEvent(wake);
            };
        }
    }

    fn anyActive(self: *const Gate) bool {
        for (self.slots) |slot| if (slot.active()) return true;
        return false;
    }

    /// The holding id while some Keeper holds the terminal.
    pub fn heldId(self: *Gate) ?[16]u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        for (self.slots) |slot| if (slot.state == .held) return self.holding_id;
        return null;
    }

    /// The slot of the one attachment that holds the terminal; null when none or both do.
    pub fn heldIndex(self: *Gate) ?usize {
        self.mutex.lock();
        defer self.mutex.unlock();
        var found: ?usize = null;
        for (self.slots, 0..) |slot, index| if (slot.state == .held) {
            if (found != null) return null;
            found = index;
        };
        return found;
    }

    /// Whether no slot is active, so no link touches this gate again: a moving terminal leaves only
    /// then, without the release `pass` would start.
    pub fn idle(self: *Gate) bool {
        self.mutex.lock();
        defer self.mutex.unlock();
        return !self.anyActive();
    }

    /// Waits until no slot is active, for the Host's teardown alone.
    pub fn awaitIdle(self: *Gate) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        while (self.anyActive()) self.condition.wait(&self.mutex);
    }

    /// The holding id the terminal was ever held under, held now or not: a moving terminal keeps
    /// naming it while no slot holds it.
    pub fn knownId(self: *Gate) ?[16]u8 {
        self.mutex.lock();
        defer self.mutex.unlock();
        return self.holding_id;
    }

    /// A restored terminal takes its record's holding id before any claim, so it names the
    /// holding it may claim and a later hold keeps the same id.
    pub fn name(self: *Gate, holding: [16]u8) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(!self.anyActive());
        self.holding_id = holding;
    }

    /// Records the holding a claim moved to this terminal: slot `index`, of the attachment
    /// `serial`, holds it under the record's holding id. A restored terminal has no active slot.
    pub fn adopt(self: *Gate, index: usize, serial: u64, wake: windows.HANDLE, holding: [16]u8) void {
        self.mutex.lock();
        defer self.mutex.unlock();
        std.debug.assert(!self.ending and !self.anyActive());
        self.holding_id = holding;
        self.slots[index] = .{ .serial = serial, .state = .held, .wake = wake };
    }
};
