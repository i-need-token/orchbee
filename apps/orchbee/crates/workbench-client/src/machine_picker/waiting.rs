//! The notice the picker owns while the hub offers a command login to this TUI: which Machine, how
//! many commands wait, and whether to log in now. Shown, it holds the keyboard and takes no key
//! but Enter and Esc, so typing meant for a Panel never reaches a login field; those two act only
//! once frames have shown the whole notice for a second. While anything else holds the keyboard
//! the notice waits and the status line names the Machine. `machine_login` owns the offers; this
//! module keeps only what the notice needs.

use super::{Choice, Picker};
use crate::client_view::{Binding, ClientView, KeyCode, KeyEvent, KeyKind, Modifiers};
use crate::machine_discovery::Discovery;
use crate::machine_login::Offered;
use crate::terminal_input::TerminalInputPort;
use orchbee_machine_protocol::Id;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

/// How long frames show a notice whole before Enter or Esc acts on it, so a key meant for a Panel
/// at the moment it opened does nothing.
const ARMING: Duration = Duration::from_secs(1);
const ACCEPT: Binding = Binding::new(KeyCode::Enter, Modifiers::NONE);
const DECLINE: Binding = Binding::new(KeyCode::Esc, Modifiers::NONE);

/// A control of the painted notice.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WaitingControl {
    Close,
    Decline,
    Accept,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WaitingNotice {
    pub(crate) offered: Offered,
    /// The last frame showed the whole notice.
    drawn: bool,
    /// The first tick that found it drawn, since it last was not.
    since: Option<Instant>,
    armed: bool,
    /// The person's choice until the shell carries it to discovery: accept or decline.
    act: Option<(Id, bool)>,
}

impl WaitingNotice {
    const fn new(offered: Offered) -> Self {
        Self {
            offered,
            drawn: false,
            since: None,
            armed: false,
            act: None,
        }
    }

    /// Whether a choice counts now: armed, not chosen yet, and not already accepted.
    const fn choosable(&self) -> bool {
        self.armed && self.act.is_none() && !self.offered.accepted
    }

    fn choose(&mut self, accept: bool) {
        if self.choosable() {
            self.act = Some((self.offered.login, accept));
        }
    }
}

impl ClientView {
    /// Whether something else holds the keyboard, so a login notice waits: the picker or a login
    /// question, another overlay, a mode, copy mode, a native capture, sidebar navigation, the Tab
    /// list or an armed prefix.
    pub fn login_notice_held(&self, port: &(impl TerminalInputPort + ?Sized)) -> bool {
        self.machines.open
            || self.machines.login.is_some()
            || self.overlay().is_some()
            || self.resize_mode()
            || self.prefix_active()
            || self.surface_owns_input()
            || port.copy_mode_active()
            || port.native_capture_claims().exclusive_any()
    }
}

impl Picker {
    pub(crate) const fn waiting(&self) -> Option<&WaitingNotice> {
        self.waiting.as_ref()
    }

    /// Whether a login notice is shown.
    pub const fn waiting_shown(&self) -> bool {
        self.waiting.is_some()
    }

    /// The Machine whose notice waits for the keyboard, for the status line.
    pub(crate) fn waiting_deferred(&self) -> Option<&str> {
        self.waiting_deferred.as_deref()
    }

    /// The name the picker shows for `machine`.
    pub(crate) fn machine_label<'a>(&'a self, machine: &'a str) -> &'a str {
        self.rows
            .iter()
            .find(|row| {
                row.context.is_none()
                    && row.agent.is_none()
                    && matches!(&row.choice, Choice::Remote(id) if id.as_str() == machine)
            })
            .map_or(machine, |row| row.label.as_str())
    }

    /// Routes a key to the shown notice, which discards every key but an armed Enter or Esc;
    /// false when none is shown.
    pub(crate) fn waiting_key(&mut self, key: KeyEvent) -> bool {
        let Some(notice) = self.waiting.as_mut() else {
            return false;
        };
        if key.kind != KeyKind::Release {
            if ACCEPT.matches(key) {
                notice.choose(true);
            } else if DECLINE.matches(key) {
                notice.choose(false);
            }
        }
        true
    }

    /// A paste while a notice is shown goes nowhere; otherwise it is given back.
    pub(crate) fn waiting_paste(&self, text: String) -> Option<String> {
        self.waiting.is_none().then_some(text)
    }

    /// A painted control of the notice for `login`; a stale one changes nothing.
    pub(crate) fn waiting_control(&mut self, login: Id, control: WaitingControl) -> bool {
        let Some(notice) = self
            .waiting
            .as_mut()
            .filter(|notice| notice.offered.login == login)
        else {
            return false;
        };
        notice.choose(control == WaitingControl::Accept);
        true
    }

    /// Records whether the frame just painted showed the whole notice.
    pub(crate) fn set_waiting_drawn(&mut self, drawn: bool) {
        if let Some(notice) = self.waiting.as_mut() {
            notice.drawn = drawn;
        }
    }

    /// The window changed: the notice counts as off screen until a frame shows it whole again,
    /// and arms anew.
    pub fn waiting_undrawn(&mut self) {
        self.set_waiting_drawn(false);
    }

    /// Lets go of the notice in a view that is no longer presented; the offer stays.
    pub fn forget_waiting(&mut self) {
        self.waiting = None;
        self.waiting_deferred = None;
    }

    /// Carries the person's choice to discovery and shows the offer discovery presents now: as a
    /// notice unless `held`, in which case one not shown yet waits and the status line names its
    /// Machine. True when anything shown changed.
    pub fn exchange_waiting(
        &mut self,
        discovery: &mut Discovery,
        held: bool,
        now: Instant,
    ) -> bool {
        if let Some((login, accept)) = self.waiting.as_mut().and_then(|notice| notice.act.take()) {
            discovery.login_offer_act(login, accept);
        }
        let before = (self.waiting.clone(), self.waiting_deferred.clone());
        let offered = discovery.login_offered();
        self.waiting = match (self.waiting.take(), offered.clone()) {
            (Some(mut notice), Some(offered)) if notice.offered.login == offered.login => {
                notice.offered = offered;
                Some(notice)
            }
            (_, Some(offered)) if !held => Some(WaitingNotice::new(offered)),
            _ => None,
        };
        self.waiting_deferred = match (&self.waiting, offered) {
            (None, Some(offered)) => Some(self.machine_label(&offered.machine).to_owned()),
            _ => None,
        };
        if let Some(notice) = self.waiting.as_mut() {
            if notice.drawn {
                let since = *notice.since.get_or_insert(now);
                notice.armed = now.saturating_duration_since(since) >= ARMING;
            } else {
                notice.since = None;
                notice.armed = false;
            }
        }
        before != (self.waiting.clone(), self.waiting_deferred.clone())
    }
}
