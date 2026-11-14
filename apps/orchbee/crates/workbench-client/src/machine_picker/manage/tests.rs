//! Each way into managing a saved Machine, as raw bytes through the real dispatcher: the Add row,
//! the form's submit and Esc, the action menu, and the remove-and-stop confirmation; and a stop
//! that fails, which keeps the record. No real host, store or SSH configuration is read.

use super::*;
use crate::client_view::{ClientActionDisposition, ClientView};
use crate::localization::ManageText;
use crate::machine_discovery::{Discovery, Observation};
use crate::machines::{Draft, Profile};
use crate::terminal_input::{
    InputDispatcher, PaneAdmission, PaneInput, SurfaceAction, SurfaceInput, TerminalInputPort,
    TerminalInputUnavailable,
};
use crate::workbench_client::input::InputAction;
use orchbee_machine_protocol::hub::StopSessionOutcome;
use std::time::Instant;

const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const DOWN: &[u8] = b"\x1b[B";
const END: &[u8] = b"\x1b[F";
const ESC: &[u8] = b"\x1b[27u";
const F2: &[u8] = b"\x1bOQ";

/// A port no management key may reach.
#[derive(Default)]
struct Port {
    reached: usize,
}

impl TerminalInputPort for Port {
    fn forward(&mut self, _input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.reached += 1;
        Ok(())
    }

    fn pane_input(&mut self, _input: PaneInput<'_>) -> PaneAdmission {
        self.reached += 1;
        PaneAdmission::Rejected
    }

    fn surface_input(&mut self, _input: SurfaceInput<'_>) -> bool {
        self.reached += 1;
        false
    }

    fn surface_action(&mut self, _action: SurfaceAction<'_>) -> Option<ClientActionDisposition> {
        None
    }

    fn apply_client_action(&mut self, _action: InputAction) -> Option<ClientActionDisposition> {
        self.reached += 1;
        None
    }

    fn open_action_menu(
        &mut self,
        _target: crate::action_menu::MenuTarget,
        _anchor: Option<crate::action_menu::MenuAnchor>,
        _pointer_released: bool,
    ) -> bool {
        self.reached += 1;
        false
    }
}

struct Bench {
    view: ClientView,
    dispatcher: InputDispatcher,
    port: Port,
    discovery: Discovery,
}

impl Bench {
    /// The picker open on the Machines list: Local, then `devbox`, which reads `observation`.
    fn open(observation: Option<Observation>) -> Self {
        let mut view = ClientView::new(100);
        let picker = &mut view.machines;
        picker.available = true;
        picker.replace(&[Profile::new(
            machine(),
            Draft::parse("devbox", "user@devbox", "default").unwrap(),
        )]);
        if let Some(observation) = observation {
            picker.observe(&[(machine(), observation)]);
        }
        picker.show();
        picker.mode = Mode::Machines;
        Self {
            view,
            dispatcher: InputDispatcher::default(),
            port: Port::default(),
            discovery: Discovery::default(),
        }
    }

    fn online() -> Self {
        Self::open(Some(Observation::State(MachineStateName::Online)))
    }

    fn send(&mut self, bytes: &[u8]) {
        let outcome = crate::terminal_input::dispatch_input(
            &mut self.dispatcher,
            &mut self.view,
            bytes,
            &mut self.port,
            Instant::now(),
            |_view, _port| Ok::<(), std::convert::Infallible>(()),
        );
        assert!(matches!(
            outcome,
            Ok(crate::terminal_input::DispatchOutcome::Continue)
        ));
        assert_eq!(self.port.reached, 0, "no management key reaches a terminal");
    }

    fn manage(&self) -> &Manage {
        &self.view.machines.manage
    }

    fn exchange(&mut self) -> exchange::Managed {
        self.view
            .machines
            .exchange_manage(&mut self.discovery, None, Instant::now())
    }

    /// Opens `devbox`'s action menu and takes its item at `position`.
    fn choose(&mut self, position: usize) {
        self.send(DOWN);
        self.send(F2);
        assert!(matches!(self.manage().page, Page::Menu { .. }));
        for _ in 0..position {
            self.send(DOWN);
        }
        self.send(b"\r");
    }
}

fn machine() -> ProfileId {
    ProfileId::parse(MACHINE).unwrap()
}

#[test]
fn the_add_row_opens_the_form_whose_enter_starts_the_checks_and_esc_gives_them_up() {
    let mut bench = Bench::open(None);
    // Past the last Machine the Add row takes the selection; Enter opens the form.
    bench.send(DOWN);
    bench.send(DOWN);
    assert!(bench.manage().add_focused);
    bench.send(b"\r");
    assert!(matches!(bench.manage().page, Page::Add(_)));
    // An empty address asks for one and starts nothing.
    bench.send(b"\r");
    let Page::Add(form) = &bench.manage().page else {
        panic!("the form stays open");
    };
    assert_eq!(form.problem, Some(Problem::Address));
    assert!(bench.manage().work.is_empty());
    // Down highlights an SSH config host and Enter takes it into the address.
    if let Page::Add(form) = &mut bench.view.machines.manage.page {
        form.offer(&["devbox2"]);
    }
    bench.send(DOWN);
    bench.send(b"\r");
    let Page::Add(form) = &bench.manage().page else {
        panic!("the form stays open");
    };
    assert_eq!(form.address.text(), "devbox2");
    bench.send(b"\r");
    let Page::Add(form) = &bench.manage().page else {
        panic!("the form stays open while the checks run");
    };
    assert_eq!(form.check, Some(Check::Starting));
    // A blank name falls back to the host name.
    assert_eq!(
        bench.manage().work,
        [Work::Add {
            label: default_label("devbox2"),
            target: "devbox2".into(),
        }]
    );
    bench.manage_work_clear();
    bench.send(ESC);
    assert_eq!(bench.manage().page, Page::List);
    assert_eq!(bench.manage().work, [Work::Discard]);
    assert!(
        bench.view.machines.open,
        "Esc leaves the form, not the picker"
    );
}

/// A failed add is given up, so after Esc the login of a selected Machine asks the person.
#[test]
fn after_a_failed_add_and_esc_a_selections_login_asks() {
    use crate::machine_login::{Effect, Notice as Login};
    let other = ProfileId::parse("fedcba9876543210fedcba9876543210").unwrap();
    let mut bench = Bench::open(None);
    bench.send(DOWN);
    bench.send(DOWN);
    bench.send(b"\r");
    bench.send(b"devbox2");
    bench.send(b"\r");
    bench.manage_work_clear();
    // A discovery that holds no hub fails every add at once.
    let draft = Draft::parse("devbox2", "devbox2", SESSION).unwrap();
    let proposal = crate::machine_add::Proposal::new(draft, &crate::machines::Catalog::default());
    bench.discovery.add(proposal.unwrap());
    bench.exchange();
    let Page::Add(form) = &bench.manage().page else {
        panic!("the form stays open on the failure");
    };
    assert!(matches!(form.check, Some(Check::Failed { .. })));
    bench.send(ESC);
    assert_eq!(bench.manage().page, Page::List);

    bench.discovery.wait_for(Some(&other));
    let login = orchbee_machine_protocol::Id::new(7).unwrap();
    let effects = bench.discovery.logins_mut().notice(
        Login::Claimed {
            login,
            machine: other.as_str().to_owned(),
        },
        |_| Some("user@other".to_owned()),
    );
    assert!(
        !effects
            .iter()
            .any(|effect| matches!(effect, Effect::Dismiss(_)))
    );
    assert!(bench.discovery.login_active(&other));
}

#[test]
fn the_menu_opens_on_f2_and_its_items_act_on_the_machine() {
    let mut bench = Bench::open(None);
    // Rename, Disable, Disconnect, Remove, Remove and stop.
    bench.choose(2);
    assert_eq!(bench.manage().page, Page::List);
    assert_eq!(bench.manage().work, [Work::Disconnect(machine())]);
    bench.manage_work_clear();
    // Rename edits the name in place; the old name is selected, so typing replaces it.
    bench.send(b"\x1b[A");
    bench.choose(0);
    assert!(matches!(bench.manage().page, Page::Rename { .. }));
    bench.send(b"lab");
    bench.send(b"\r");
    assert_eq!(
        bench.manage().work,
        [Work::Rename {
            machine: machine(),
            label: "lab".into(),
        }]
    );
    // A Machine that is not online cannot have its Session stopped.
    bench.send(b"\x1b[A");
    bench.send(DOWN);
    bench.send(F2);
    bench.send(END);
    bench.send(b"\r");
    assert!(matches!(bench.manage().page, Page::Menu { .. }));
}

#[test]
fn the_confirmation_stops_on_enter_and_goes_back_on_esc() {
    let mut bench = Bench::online();
    bench.choose(4);
    assert!(matches!(bench.manage().page, Page::Confirm { .. }));
    bench.send(ESC);
    assert_eq!(bench.manage().page, Page::List);
    assert!(bench.manage().work.is_empty());
    bench.send(b"\x1b[A");
    bench.choose(4);
    bench.send(b"\r");
    assert_eq!(bench.manage().work, [Work::Stop(machine())]);
    assert!(bench.manage().stopping(&machine()));
}

#[test]
fn a_failed_stop_keeps_the_record() {
    let mut bench = Bench::online();
    bench.choose(4);
    bench.send(b"\r");
    // The first exchange asks the hub to stop the Session; the hub reports it failed.
    assert!(bench.exchange().catalog.is_none());
    bench
        .discovery
        .settle_stop(&machine(), Some(StopSessionOutcome::Failed));
    let managed = bench.exchange();
    assert!(managed.catalog.is_none(), "nothing was committed");
    assert!(bench.manage().work.is_empty());
    assert!(bench.manage().record(&machine()).is_some());
    assert_eq!(bench.manage().page, Page::List);
    assert_eq!(
        bench.manage().notice,
        Some(Notice::StopFailed("devbox".into()))
    );
}

impl Bench {
    fn manage_work_clear(&mut self) {
        self.view.machines.manage.work.clear();
    }

    /// The frame's text with spaces and frame glyphs gone, so wrapped words read as one run.
    fn paint(&self, locale: crate::localization::Locale) -> String {
        let mut picker = self.view.machines.clone();
        let backend = ratatui::backend::TestBackend::new(80, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut hits = crate::tui_hit_map::HitMapBuilder::new();
        terminal
            .draw(|frame| {
                picker.render(
                    frame,
                    locale,
                    &crate::tui_theme::Theme::terminal_native(),
                    &mut hits,
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
            .chars()
            .filter(|c| !c.is_whitespace() && !('\u{2500}'..='\u{257f}').contains(c))
            .collect()
    }
}

fn squeezed(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn each_page_names_what_it_acts_on_in_both_locales() {
    use crate::localization::Locale;
    let mut empty = Bench::open(None);
    empty.view.machines.replace(&[]);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = empty.paint(locale);
        assert!(text.contains(&squeezed(
            locale.machine_manage_text(ManageText::EmptyState)
        )));
        assert!(text.contains(&squeezed(
            locale.machine_manage_text(ManageText::AddMachine)
        )));
    }
    let mut bench = Bench::open(None);
    bench.send(DOWN);
    let text = bench.paint(Locale::EnUs);
    assert!(text.contains("user@devbox"), "the row names its address");
    assert!(text.contains("Actions"));
    // Remove and stop is unavailable while devbox is not online, and the menu says why.
    bench.send(F2);
    bench.send(END);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        assert!(
            bench
                .paint(locale)
                .contains(&squeezed(&locale.machine_cannot_stop("devbox")))
        );
    }
    let mut online = Bench::online();
    online.choose(4);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = online.paint(locale);
        assert!(text.contains(&squeezed(
            locale.machine_manage_text(ManageText::ConfirmTitle)
        )));
        // No overview counts devbox's terminals, so the line names none.
        assert!(text.contains(&squeezed(&locale.machine_ends_terminals(None, "devbox"))));
    }
    online.send(ESC);
    online.send(DOWN);
    online.send(b"\r");
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let text = online.paint(locale);
        assert!(text.contains(&squeezed(locale.machine_manage_text(ManageText::NameHint))));
    }
}
