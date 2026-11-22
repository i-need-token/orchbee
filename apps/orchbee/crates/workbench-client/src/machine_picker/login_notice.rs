//! The status line's login notice: saved machines that need a login nobody is asked for, named as
//! one notice for all of them. It never takes the keyboard; it names the key that opens Settings ›
//! Machines, and showing that page puts it away until another machine needs a login.

use super::{Choice, Picker, Row};

/// What the login notice names.
pub(crate) enum LoginNotice<'a> {
    One(&'a str),
    Many(usize),
}

impl Picker {
    /// Saved machines that need a login, except the one whose card says so already.
    fn needing_login(&self) -> impl Iterator<Item = &Row> {
        let card = self.card_machine();
        self.rows.iter().filter(move |row| {
            row.enabled
                && row.context.is_none()
                && row.agent.is_none()
                && matches!(&row.choice, Choice::Remote(id) if card.as_ref() != Some(id))
                && row
                    .observation
                    .as_ref()
                    .is_some_and(|observation| observation.key() == "needs-login")
        })
    }

    /// The notice to name, while one of the machines that need a login was not seen on Settings ›
    /// Machines since it began to.
    pub(crate) fn login_notice(&self) -> Option<LoginNotice<'_>> {
        let needing: Vec<&Row> = self.needing_login().collect();
        if needing
            .iter()
            .all(|row| self.login_seen.contains(&row.choice))
        {
            return None;
        }
        Some(match needing.as_slice() {
            [one] => LoginNotice::One(&one.label),
            many => LoginNotice::Many(many.len()),
        })
    }

    /// Takes whether Settings › Machines shows, which counts as seeing every machine that needs a
    /// login; a machine that no longer needs one is forgotten, so it is named again when it does.
    /// True when the notice changed.
    pub fn observe_login_notice(&mut self, settings_shown: bool) -> bool {
        let before = self.login_notice().is_some();
        let needing: Vec<Choice> = self.needing_login().map(|row| row.choice.clone()).collect();
        if settings_shown {
            self.login_seen = needing;
        } else {
            self.login_seen.retain(|choice| needing.contains(choice));
        }
        before != self.login_notice().is_some()
    }
}
