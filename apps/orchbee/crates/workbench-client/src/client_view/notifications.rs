//! Client-local transient notification card state: delivery policy, the one visible card, and
//! the one pending outside effect. Painting, transport, and host effects live in `notifications`.
use crate::machine_agents::checkpoint::Target;
use orchbee_local_runtime::{NotificationDelivery as Delivery, NotificationSettings};
use orchbee_workbench_protocol::{
    Notification, NotificationPosition as Position, NotificationSound,
};
use std::time::{Duration, Instant};
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Effect {
    pub delivery: Delivery,
    pub message: Notification,
}
/// Names one installed card, so a pointer gesture opens only the card it pressed. Tokens are
/// unique in the process, so an expired card leaves no trace in the notification state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CardToken(u64);
impl CardToken {
    fn next() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
/// How the Workbench answered a request to open an Agent notification's target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetOpening {
    /// The target Panel is focused now or its navigation was admitted.
    Opened,
    /// The user's own switch or a background read is settling; the opening waits for it and then
    /// runs with every fact checked again.
    Held,
    /// Other work keeps the Workbench from navigating; the busy notice answers and the card stays
    /// for another attempt.
    Busy,
    /// The target is gone, rebound, or outside the attached Session; the notice says so and
    /// nothing moved.
    Unavailable,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Visible {
    message: Notification,
    until: Instant,
    sound_pending: bool,
    token: CardToken,
    /// The Agent an automatic notification announced, as observed when it was raised. Custom
    /// notifications have none.
    target: Option<Box<Target>>,
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
    /// The message of the card painted now, if any.
    pub(crate) fn visible_message(&self) -> Option<&Notification> {
        self.visible.as_ref().map(|visible| &visible.message)
    }
    /// The visible card's Agent target, if the card announces an Agent.
    pub(crate) fn agent_card(&self) -> Option<(CardToken, &Target)> {
        let visible = self.visible.as_ref()?;
        Some((visible.token, visible.target.as_deref()?))
    }
    /// Removes the card this token names; a newer card stays.
    pub(crate) fn dismiss(&mut self, token: CardToken) -> bool {
        if self.visible.as_ref().is_some_and(|v| v.token == token) {
            self.visible = None;
            true
        } else {
            false
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
        self.install_with_delivery(message, self.policy.delivery, None, now)
    }
    pub(crate) fn install_automatic(
        &mut self,
        message: Notification,
        delivery: Delivery,
        target: Option<Target>,
        now: Instant,
    ) -> bool {
        self.install_with_delivery(message, delivery, target, now)
    }
    fn install_with_delivery(
        &mut self,
        mut message: Notification,
        delivery: Delivery,
        target: Option<Target>,
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
            token: CardToken::next(),
            target: target.map(Box::new),
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
