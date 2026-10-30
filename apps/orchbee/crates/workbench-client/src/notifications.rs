//! Main-TUI transient notification state. Its card painter lives in `paint`, its transport in
//! `subscription`, and its host effects in `system`.
use orchbee_workbench_protocol::{
    Notification, NotificationPosition as Position, NotificationSound,
};
use std::time::{Duration, Instant};
pub(crate) mod paint;
pub(crate) mod subscription;
pub(crate) mod system;
pub(crate) mod terminal;
use orchbee_local_runtime::{NotificationDelivery as Delivery, NotificationSettings};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Effect {
    pub delivery: Delivery,
    pub message: Notification,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Visible {
    message: Notification,
    until: Instant,
    sound_pending: bool,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct State {
    visible: Option<Visible>,
    policy: NotificationSettings,
    pending: Option<Effect>,
}
impl State {
    pub fn configure(&mut self, policy: NotificationSettings) {
        if self.policy != policy {
            self.visible = None;
            self.pending = None;
            self.policy = policy;
        }
    }
    pub(crate) fn card_delivery(&self) -> bool {
        self.policy.delivery == Delivery::Tui
    }
    pub fn policy(&self) -> &NotificationSettings {
        &self.policy
    }
    pub fn take_effect(&mut self) -> Option<Effect> {
        self.pending.take()
    }
    pub(crate) fn tick(&mut self, now: Instant) -> bool {
        if self.visible.as_ref().is_some_and(|v| now >= v.until) {
            self.visible = None;
            true
        } else {
            false
        }
    }
    pub fn empty(&self) -> bool {
        self.visible.is_none() && self.pending.is_none()
    }
    pub(crate) fn install(&mut self, message: Notification, now: Instant) -> bool {
        self.install_with_delivery(message, self.policy.delivery, now)
    }
    pub(crate) fn install_automatic(
        &mut self,
        message: Notification,
        delivery: Delivery,
        now: Instant,
    ) -> bool {
        self.install_with_delivery(message, delivery, now)
    }
    fn install_with_delivery(
        &mut self,
        mut message: Notification,
        delivery: Delivery,
        now: Instant,
    ) -> bool {
        if !self.empty() {
            return false;
        }
        if delivery != Delivery::Tui {
            self.pending = Some(Effect { delivery, message });
            return true;
        }
        message.position = message.position.or(Some(match self.policy.position() {
            "top-left" => Position::TopLeft,
            "top-right" => Position::TopRight,
            "bottom-left" => Position::BottomLeft,
            _ => Position::BottomRight,
        }));
        self.visible = Some(Visible {
            message,
            until: now + Duration::from_secs(5),
            sound_pending: true,
        });
        true
    }
    pub fn take_sound_after_draw(&mut self) -> NotificationSound {
        match self.visible.as_mut() {
            Some(v) => {
                if std::mem::take(&mut v.sound_pending) {
                    v.message.sound
                } else {
                    NotificationSound::None
                }
            }
            _ => NotificationSound::None,
        }
    }
}
/// Crate-internal `State` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait StateTestAccess {
    fn install(&mut self, message: Notification, now: Instant) -> bool;
}

#[cfg(any(test, feature = "test-support"))]
impl StateTestAccess for State {
    fn install(&mut self, message: Notification, now: Instant) -> bool {
        State::install(self, message, now)
    }
}

#[cfg(test)]
mod tests;
