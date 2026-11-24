//! The desktop window's saved machines, kept by a headless process the window's Main starts: the
//! machine hub held as the TUI holds it, the catalog read each second, Settings › Machines' facts
//! worded in the page's locale, and the page's requests carried out as the TUI's page carries
//! them. `apps/orchbee/docs/machines.md#desktop-machines-process` owns the contract.
//
// module-layer: composition

mod intent;
mod login;
mod view;

pub use intent::{Intent, LoginAct, parse_intent, parse_locale};

use crate::{
    localization::Locale,
    machine_discovery::Discovery,
    machine_login::Act,
    machine_settings_exchange::Exchange,
    machines::{Catalog, Error, Profile, Store},
    ui_wakeup::UiWakeup,
};
use serde_json::Value;
use std::{
    io,
    time::{Duration, Instant},
};

/// How often the catalog is read again, as the TUI reads it.
const REFRESH: Duration = Duration::from_secs(1);

pub struct DesktopMachines {
    profiles: Vec<Profile>,
    /// The catalog's last read failed.
    unreadable: bool,
    refresh: Instant,
    discovery: Discovery,
    settings: Exchange,
    locale: Locale,
    /// The page shows what the machines have free.
    capacity: bool,
    /// How the last request ended, worded, and its number; the page says each number once.
    notice: Option<(u64, String)>,
}

impl DesktopMachines {
    /// Holds the hub once the catalog holds a machine it can observe, and wakes `wakeup` when the
    /// hub reports a change.
    #[must_use]
    pub fn new(wakeup: UiWakeup, locale: Locale) -> Self {
        Self {
            profiles: Vec::new(),
            unreadable: false,
            refresh: Instant::now(),
            discovery: Discovery::new(wakeup),
            settings: Exchange::default(),
            locale,
            capacity: false,
            notice: None,
        }
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    /// Whether the page shows what the machines have free, which is asked only meanwhile.
    pub fn watch_capacity(&mut self, watching: bool) {
        self.capacity = watching;
    }

    /// When the next tick is due without a wake.
    #[must_use]
    pub const fn due(&self) -> Instant {
        self.refresh
    }

    /// One pass: the catalog when due, the hub's news, what earlier requests settled, and the
    /// whole view the page shows now.
    pub fn tick(&mut self, now: Instant) -> Value {
        if now >= self.refresh {
            self.refresh = now + REFRESH;
            self.refresh_catalog(Store::configured().and_then(|store| store.read()), now);
        }
        self.discovery.tick(now);
        self.settings
            .settle(&mut self.discovery, &self.profiles, now);
        // Only the login the page asked for may ask the person; any other is dismissed.
        self.discovery.wait_for(self.settings.login());
        if self.capacity {
            self.discovery.sample_capacity(now);
        }
        let facts = self
            .settings
            .facts(&mut self.discovery, &self.profiles, |_| None, now);
        if let Some(outcome) = &facts.outcome {
            let number = self.notice.as_ref().map_or(1, |(number, _)| number + 1);
            self.notice = Some((number, self.locale.machine_outcome(outcome)));
        }
        let login = self.discovery.login_presented().map(|presented| {
            let label = self
                .profiles
                .iter()
                .find(|profile| profile.id.as_str() == presented.machine)
                .map_or(presented.machine.as_str(), |profile| {
                    profile.draft.label.as_str()
                });
            login::project(self.locale, &presented, label)
        });
        self.discovery.finish_retirement();
        view::project(
            self.locale,
            &facts,
            self.discovery.hub_message(),
            self.unreadable,
            self.notice.as_ref(),
            login,
        )
    }

    /// Carries out the page's `intent`; a commit's catalog takes effect at once.
    pub fn carry(&mut self, intent: Intent, now: Instant) {
        let request = match intent {
            Intent::Settings(request) => request,
            Intent::Login {
                login,
                question,
                act,
            } => return self.carry_login((login, question), act),
        };
        let store = Store::configured().ok();
        let carried = self.settings.carry(
            request,
            &mut self.discovery,
            store.as_ref(),
            &self.profiles,
            now,
        );
        if let Some(catalog) = carried.catalog {
            self.refresh_catalog(Ok(catalog), now);
        }
    }

    /// The person's act on the question the view showed; one on a question no longer shown, or an
    /// answer the field cannot take whole, changes nothing.
    fn carry_login(&mut self, ids: (u32, u32), act: LoginAct) {
        let Some(presented) = self
            .discovery
            .login_presented()
            .filter(|presented| presented.token.ids() == ids)
        else {
            return;
        };
        let token = presented.token;
        let act = match act {
            LoginAct::Answer { answer, remember } => {
                presented.field.clear();
                if !answer
                    .characters()
                    .all(|character| presented.field.push(character))
                {
                    presented.field.clear();
                    return;
                }
                // The episode learns the box before the answer leaves.
                if let Some(ticked) = remember {
                    self.discovery.login_remember(token, ticked);
                }
                Act::Submit
            }
            LoginAct::Trust => Act::Trust,
            LoginAct::Cancel => Act::Cancel,
            LoginAct::Close => Act::Close,
        };
        self.discovery.login_act(token, act);
    }

    fn refresh_catalog(&mut self, catalog: Result<Catalog, Error>, now: Instant) {
        match catalog {
            Ok(catalog) => {
                self.profiles = catalog.profiles;
                self.unreadable = false;
                self.discovery
                    .reconcile(&self.profiles, Some(catalog.revision), now);
            }
            Err(_) => {
                self.unreadable = true;
                self.discovery.reconcile(&[], None, now);
            }
        }
    }

    /// Asks every machine observer and the holder to stop, and waits for them until `deadline`.
    pub fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.discovery.stop_before(deadline)
    }
}
