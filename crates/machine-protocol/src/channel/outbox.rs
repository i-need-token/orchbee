//! The frames one end has queued and the order it writes them in: every frame other than `data`
//! first, in queue order, with a newer state event replacing the undelivered one of its subject in
//! place; then one `data` frame of each ready channel per round; and `bye` last of all.

use std::collections::VecDeque;

use crate::Frame;

/// A queued frame other than `data`.
#[derive(Debug)]
pub(super) struct Queued {
    pub(super) frame: Frame,
    /// The state event's subject, whose newest copy replaces older undelivered ones.
    subject: Option<String>,
    /// The frame is a `pong`, owed to the peer until written.
    pub(super) pong: bool,
}

#[derive(Debug, Default)]
pub(super) struct Outbox {
    control: VecDeque<Queued>,
    ready: VecDeque<u32>,
    bye: Option<Frame>,
    finished: bool,
}

impl Outbox {
    /// Queues a frame other than `data`, `pong`, a state event, or `bye`.
    pub(super) fn push(&mut self, frame: Frame) {
        self.control.push_back(Queued {
            frame,
            subject: None,
            pong: false,
        });
    }

    /// Queues the `pong` of a received `ping`.
    pub(super) fn push_pong(&mut self, frame: Frame) {
        self.control.push_back(Queued {
            frame,
            subject: None,
            pong: true,
        });
    }

    /// Queues an event; with a subject, it replaces that subject's undelivered event in place.
    pub(super) fn push_event(&mut self, frame: Frame, subject: Option<String>) {
        if let Some(subject) = &subject
            && let Some(slot) = self
                .control
                .iter_mut()
                .find(|queued| queued.subject.as_ref() == Some(subject))
        {
            slot.frame = frame;
            return;
        }
        self.control.push_back(Queued {
            frame,
            subject,
            pong: false,
        });
    }

    /// Gives channel `id` a turn in the `data` round.
    pub(super) fn mark_ready(&mut self, id: u32) {
        self.ready.push_back(id);
    }

    /// The next frame other than `data`.
    pub(super) fn pop_control(&mut self) -> Option<Queued> {
        self.control.pop_front()
    }

    /// The next channel whose turn it is.
    pub(super) fn pop_ready(&mut self) -> Option<u32> {
        self.ready.pop_front()
    }

    /// Queues `bye`, written after every other queued frame.
    pub(super) fn set_bye(&mut self, frame: Frame) {
        if !self.finished {
            self.bye = Some(frame);
        }
    }

    /// Drops every queued frame and leaves only `bye`, unless one was already written.
    pub(super) fn fail(&mut self, bye: Frame) {
        self.control.clear();
        self.ready.clear();
        self.set_bye(bye);
    }

    /// `bye`, once every other frame was written.
    pub(super) fn take_bye(&mut self) -> Option<Frame> {
        let bye = self.bye.take();
        if bye.is_some() {
            self.finished = true;
        }
        bye
    }

    /// Whether `bye` was written, so the carrier may close.
    pub(super) const fn is_finished(&self) -> bool {
        self.finished
    }

    /// The queued frames other than `data` and `bye`.
    pub(super) fn control_len(&self) -> usize {
        self.control.len()
    }

    /// Whether no frame waits to be written.
    pub(super) fn is_empty(&self) -> bool {
        self.control.is_empty() && self.ready.is_empty() && self.bye.is_none()
    }
}
