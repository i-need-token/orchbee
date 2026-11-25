//! The desktop window's saved machines, kept by a headless process the window's Main starts: the
//! machine hub held as the TUI holds it, the catalog read each second, Settings › Machines' facts
//! worded in the page's locale, and the page's requests carried out as the TUI's page carries
//! them. `apps/orchbee/docs/machines.md#desktop-machines-process` owns the contract.
//
// module-layer: composition

mod add;
mod intent;
mod login;
mod moving;
mod offer;
mod view;

pub use intent::{Intent, LoginAct, parse_intent, parse_locale};
pub use moving::{move_notice, move_pending, move_question};

use add::Add;

use crate::{
    localization::Locale,
    machine_color::MachineColor,
    machine_discovery::Discovery,
    machine_login::Act,
    machine_observation::{Source, outcome::Observation},
    machine_settings_exchange::Exchange,
    machine_spaces::{Checkpoint, last_seen::LastSeen},
    machines::{Catalog, Choice, Error, Profile, ProfileId, Store},
    ui_wakeup::UiWakeup,
};
use serde_json::Value;
use std::{
    io,
    sync::Arc,
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
    /// Add Machine, while the page shows it.
    add: Option<Add>,
    /// Each online machine's Workspaces as its Session answers, read as the TUI's sidebar reads
    /// them, and each one's last answer this run.
    spaces: crate::machine_spaces::Collector,
    fresh: Vec<(ProfileId, Arc<Checkpoint>)>,
    /// What this computer last read of every machine's Workspaces, kept across runs and shared
    /// with the TUI.
    last_seen: LastSeen,
}

impl DesktopMachines {
    /// Holds the hub once the catalog holds a machine it can observe, and wakes `wakeup` when the
    /// hub reports a change.
    #[must_use]
    pub fn new(wakeup: UiWakeup, locale: Locale) -> Self {
        let spaces = crate::machine_spaces::Collector::new(wakeup.clone(), false);
        let mut discovery = Discovery::new(wakeup);
        // Desktop shows command logins only while it has a window.
        discovery.set_asks(false);
        Self {
            profiles: Vec::new(),
            unreadable: false,
            refresh: Instant::now(),
            discovery,
            settings: Exchange::default(),
            locale,
            capacity: false,
            notice: None,
            add: None,
            spaces,
            fresh: Vec::new(),
            last_seen: LastSeen::configured(),
        }
    }

    /// Reads each online machine's Workspaces and keeps the last-seen list current, as the TUI
    /// does, so a machine that is not connected still lists them in any Client.
    fn observe_spaces(&mut self, now: Instant) {
        let sources = self
            .profiles
            .iter()
            .filter_map(|profile| {
                Some(Source {
                    choice: Choice::Remote(profile.id.clone()),
                    connection: self.discovery.connection(profile, now)?,
                })
            })
            .collect();
        let observed = self
            .spaces
            .tick(sources, now, crate::machine_spaces::read_metadata);
        let mut fresh = Vec::new();
        for (choice, observation) in observed {
            let Choice::Remote(id) = choice else {
                continue;
            };
            let checkpoint = match observation {
                Observation::Ready(checkpoint) => Some(checkpoint),
                // A read in flight keeps the answer before it.
                Observation::Loading => self
                    .fresh
                    .iter()
                    .find(|(known, _)| known == &id)
                    .map(|(_, checkpoint)| checkpoint.clone()),
                Observation::Unavailable => None,
            };
            if let Some(checkpoint) = checkpoint {
                fresh.push((id, checkpoint));
            }
        }
        self.fresh = fresh;
        // A catalog that could not be read names no machine, and would empty the list.
        if !self.unreadable {
            let saved: Vec<_> = self
                .profiles
                .iter()
                .map(|profile| profile.id.clone())
                .collect();
            self.last_seen.observe(
                self.fresh
                    .iter()
                    .map(|(id, checkpoint)| (id, checkpoint.as_ref())),
                &saved,
                now,
            );
        }
    }

    pub fn set_locale(&mut self, locale: Locale) {
        self.locale = locale;
    }

    /// The page's locale, in which every view is worded.
    #[must_use]
    pub const fn locale(&self) -> Locale {
        self.locale
    }

    /// Whether Desktop has a window, which alone shows the command logins the hub offers.
    pub fn set_window(&mut self, exists: bool) {
        self.discovery.set_asks(exists);
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
        if let Some(add) = &mut self.add {
            let store = add.running().then(|| Store::configured().ok()).flatten();
            if let Some(catalog) = add.follow(&mut self.discovery, store.as_ref(), now) {
                self.refresh_catalog(Ok(catalog), now);
            }
        }
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
            login::project(self.locale, &presented, self.label(&presented.machine))
        });
        let offer = self
            .discovery
            .login_offered()
            .map(|offered| offer::project(self.locale, &offered, self.label(&offered.machine)));
        self.observe_spaces(now);
        let enabled: Vec<_> = self
            .profiles
            .iter()
            .filter(|profile| profile.enabled)
            .map(|profile| profile.id.clone())
            .collect();
        self.discovery.finish_retirement();
        view::project(
            self.locale,
            &facts,
            self.discovery.hub_message(),
            self.unreadable,
            self.notice.as_ref(),
            view::Projected {
                login,
                offer,
                seen: view::seen(&self.last_seen.checkpoints(&enabled)),
            },
            self.add.as_ref(),
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
            Intent::AddOpen => {
                self.close_add();
                self.add = Some(Add::open(
                    now,
                    self.discovery.wakeup(),
                    self.default_color(),
                ));
                return;
            }
            Intent::Add { address, color } => {
                let store = Store::configured().ok();
                let wakeup = self.discovery.wakeup().cloned();
                let first = self.default_color();
                self.add
                    .get_or_insert_with(|| Add::open(now, wakeup.as_ref(), first))
                    .start(&mut self.discovery, store.as_ref(), &address, color);
                return;
            }
            Intent::AddClose => return self.close_add(),
            // The process's mover runs moves, as the TUI shell's does.
            Intent::Move { .. } | Intent::MoveAnswer { .. } => return,
            Intent::Offer { login, accept } => {
                // A choice on a notice no longer shown changes nothing.
                if let Some(offered) = self
                    .discovery
                    .login_offered()
                    .filter(|offered| offered.login.get() == login)
                {
                    self.discovery.login_offer_act(offered.login, accept);
                }
                return;
            }
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

    /// The color a machine added now takes: the one the fewest saved machines use.
    fn default_color(&self) -> MachineColor {
        MachineColor::least_used(self.profiles.iter().map(|profile| profile.color))
    }

    /// The name `machine` is shown by: its record's, or, for the machine being added, the one the
    /// person typed; never its id while either is known.
    /// The name of the saved machine `machine`, or `machine` itself when none is saved by it.
    #[must_use]
    pub fn machine_label(&self, machine: &str) -> String {
        self.label(machine).to_owned()
    }

    fn label<'a>(&'a self, machine: &'a str) -> &'a str {
        self.profiles
            .iter()
            .find(|profile| profile.id.as_str() == machine)
            .map(|profile| profile.draft.label.as_str())
            .or_else(|| {
                self.add
                    .as_ref()
                    .filter(|_| {
                        self.discovery
                            .proposal()
                            .is_some_and(|proposal| proposal.id.as_str() == machine)
                    })
                    .map(|add| add.label.as_str())
            })
            .unwrap_or(machine)
    }

    /// Closes Add Machine; an add still checking is given up with its draft, link and login. The
    /// person gave it up, so its login closes too instead of asking to be closed.
    fn close_add(&mut self) {
        if !self
            .add
            .take()
            .is_some_and(|add| add.running() && add.saved.is_none())
        {
            return;
        }
        let machine = self
            .discovery
            .proposal()
            .map(|proposal| proposal.id.as_str().to_owned());
        self.discovery.discard_add();
        if let Some(presented) = self
            .discovery
            .login_presented()
            .filter(|presented| machine.as_deref() == Some(presented.machine.as_str()))
        {
            self.discovery.login_act(presented.token, Act::Close);
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

    /// Asks every machine observer, every Workspace read and the holder to stop, and waits for them
    /// until `deadline`.
    pub fn stop_before(&mut self, deadline: Instant) -> io::Result<()> {
        self.spaces.request_stop();
        let spaces = self.spaces.stop_before(deadline);
        self.discovery.stop_before(deadline).and(spaces)
    }
}
