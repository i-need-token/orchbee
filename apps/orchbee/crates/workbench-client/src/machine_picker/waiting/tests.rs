//! Scripted runs of a login notice through the real input path: a typist's keys and a paste,
//! however long they run, Enter (`CR`) and Esc once a second passed without a dropped key, a notice
//! that waits while the picker holds the keyboard, and an accepted login that ends before its
//! question.

use super::*;
use crate::client_view::{ClientActionDisposition, ClientView};
use crate::localization::Locale;
use crate::machine_login::{Notice, Withdrawn};
use crate::terminal_input::{
    CopyAction, InputDispatcher, MenuAction, PaneAdmission, PaneInput, SurfaceAction, SurfaceInput,
    TerminalInputPort, TerminalInputUnavailable,
};
use crate::tui_hit_map::HitMapBuilder;
use crate::tui_theme::Theme;
use crate::workbench_client::input::InputAction;
use orchbee_machine_protocol::{
    AnswerToken, PathText, Prompt,
    hub::{AnswerEndpoint, Question},
};

const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const TARGET: &str = "user@devbox";

/// The terminal port: records every input that would reach a pane, a surface or an action.
#[derive(Default)]
struct Port {
    forwarded: Vec<u8>,
    reached: Vec<&'static str>,
}

impl TerminalInputPort for Port {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }

    fn pane_input(&mut self, _input: PaneInput<'_>) -> PaneAdmission {
        self.reached.push("pane_input");
        PaneAdmission::Rejected
    }

    fn surface_input(&mut self, _input: SurfaceInput<'_>) -> bool {
        self.reached.push("surface_input");
        false
    }

    fn surface_action(&mut self, action: SurfaceAction<'_>) -> Option<ClientActionDisposition> {
        // Ending a menu or copy mode under a higher capture is the dispatcher's housekeeping,
        // not input.
        if !matches!(
            action,
            SurfaceAction::ActionMenu(MenuAction::Cancel)
                | SurfaceAction::CopyMode(CopyAction::Exit)
        ) {
            self.reached.push("surface_action");
        }
        None
    }

    fn apply_client_action(&mut self, _action: InputAction) -> Option<ClientActionDisposition> {
        self.reached.push("apply_client_action");
        None
    }

    fn open_action_menu(
        &mut self,
        _target: crate::action_menu::MenuTarget,
        _anchor: Option<crate::action_menu::MenuAnchor>,
        _pointer_released: bool,
    ) -> bool {
        self.reached.push("open_action_menu");
        false
    }

    fn copy_picker_identifier(&mut self, _text: String) -> bool {
        self.reached.push("copy_picker_identifier");
        false
    }
}

fn id(value: u32) -> Id {
    Id::new(value).unwrap()
}

/// One view over the real dispatcher, the episodes the shell carries to it, and the shell's clock.
struct Bench {
    view: ClientView,
    dispatcher: InputDispatcher,
    port: Port,
    discovery: Discovery,
    now: Instant,
}

impl Bench {
    /// The hub offers login 1 of `MACHINE`, which two commands wait on; nothing is selected.
    fn offered() -> Self {
        let mut bench = Self {
            view: ClientView::new(100),
            dispatcher: InputDispatcher::default(),
            port: Port::default(),
            discovery: Discovery::default(),
            now: Instant::now(),
        };
        bench.view.machines.available = true;
        bench.notice(Notice::Offered {
            login: id(1),
            machine: MACHINE.into(),
            waiters: 2,
        });
        bench
    }

    fn notice(&mut self, notice: Notice) {
        let logins = self.discovery.logins_mut();
        logins.notice(notice, |machine| {
            (machine == MACHINE).then(|| TARGET.to_owned())
        });
    }

    /// What the shell's tick does `after` the last one.
    fn tick(&mut self, after: Duration) {
        self.now += after;
        let held = self.view.login_notice_held(&self.port);
        self.view.machines.exchange_login(&mut self.discovery);
        self.view
            .machines
            .exchange_waiting(&mut self.discovery, held, self.now);
    }

    fn send(&mut self, bytes: &[u8]) {
        let outcome = crate::terminal_input::dispatch_input(
            &mut self.dispatcher,
            &mut self.view,
            bytes,
            &mut self.port,
            self.now,
            |_view, _port| Ok::<(), std::convert::Infallible>(()),
        );
        assert!(matches!(
            outcome,
            Ok(crate::terminal_input::DispatchOutcome::Continue)
        ));
    }

    /// The frame's text with its spaces and frame glyphs gone, so wrapped text reads as one run.
    fn paint(&mut self, locale: Locale) -> String {
        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        let mut hits = HitMapBuilder::new();
        let theme = Theme::terminal_native();
        terminal
            .draw(|frame| self.view.machines.render(frame, locale, &theme, &mut hits))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect::<String>()
            .chars()
            .filter(|character| {
                !character.is_whitespace() && !('\u{2500}'..='\u{257f}').contains(character)
            })
            .collect()
    }

    fn notice_shown(&self) -> &WaitingNotice {
        self.view.machines.waiting().expect("the notice is shown")
    }

    fn untouched(&self) {
        assert!(self.port.forwarded.is_empty());
        assert_eq!(self.port.reached, [""; 0]);
    }
}

#[test]
fn a_notice_takes_only_an_armed_enter_or_esc_from_the_real_input_path() {
    let squeezed = |text: &str| text.replace(' ', "");
    let mut bench = Bench::offered();
    bench.tick(Duration::ZERO);
    assert!(bench.view.machines.captures());
    let english = bench.paint(Locale::EnUs);
    for line in [
        "2 commands are waiting to log in to this machine.",
        "If you don't log in now, these commands will fail.",
        "Not now",
        "Log in",
    ] {
        assert!(english.contains(&squeezed(line)), "{line}\n{english}");
    }
    let chinese = bench.paint(Locale::ZhCn);
    for line in [
        "有 2 条命令在等待登录这台机器。",
        "暂不登录的话，这些命令会失败。",
    ] {
        assert!(chinese.contains(&squeezed(line)), "{line}\n{chinese}");
    }

    // A typist's keys, every Enter among them, and a paste go nowhere and accept nothing however
    // long the typing runs past the first second: each dropped key starts the second again.
    for _ in 0..12 {
        bench.tick(Duration::from_millis(400));
        bench.send(b"ls -la\r");
    }
    bench.send(b"\x1b[200~pasted\x1b[201~");
    bench.tick(Duration::from_millis(900));
    bench.send(b"\r");
    bench.tick(Duration::ZERO);
    assert!(!bench.notice_shown().offered.accepted);
    bench.untouched();

    // A second with no key dropped arms it: `CR` is Enter, which accepts; later keys change
    // nothing.
    bench.tick(Duration::from_secs(1));
    bench.send(b"\r");
    bench.tick(Duration::ZERO);
    assert!(bench.notice_shown().offered.accepted);
    bench.send(b"x\x1b[27u");
    bench.tick(Duration::ZERO);
    assert!(bench.notice_shown().offered.accepted);
    bench.untouched();

    // The hub makes the login this TUI's and puts its question: the login overlay replaces it.
    bench.notice(Notice::Claimed {
        login: id(1),
        machine: MACHINE.into(),
    });
    bench.notice(Notice::Question(Question {
        login: id(1),
        question: id(1),
        kind: crate::machine_question::classify("user@devbox's password: "),
        prompt: Prompt::new("user@devbox's password: ").unwrap(),
        repeated: false,
        remembered_rejected: false,
        answer: AnswerEndpoint {
            endpoint: PathText::new("answer").unwrap(),
            token: AnswerToken::from_bytes([7; 16]),
        },
    }));
    bench.tick(Duration::ZERO);
    assert!(bench.view.machines.login().is_some());
    assert!(!bench.view.machines.waiting_shown());

    // Another offer waits while the picker holds the keyboard, and the status names the Machine.
    let mut bench = Bench::offered();
    bench.view.machines.show();
    bench.tick(Duration::ZERO);
    assert!(!bench.view.machines.waiting_shown());
    assert_eq!(bench.view.machines.waiting_deferred(), Some(MACHINE));
    bench.view.machines.close_picker();
    bench.tick(Duration::ZERO);
    assert_eq!(bench.view.machines.waiting_deferred(), None);
    bench.paint(Locale::EnUs);
    bench.tick(Duration::ZERO);
    bench.tick(Duration::from_secs(1));
    // Esc, in its unambiguous form, declines: the offer goes and nothing holds the keys.
    bench.send(b"\x1b[27u");
    bench.tick(Duration::ZERO);
    assert!(!bench.view.machines.waiting_shown());
    assert!(bench.discovery.login_offered().is_none());
    assert!(!bench.view.machines.captures());
    bench.untouched();
}

#[test]
fn an_accepted_login_that_ends_first_holds_the_keys_until_the_person_closes_it() {
    let squeezed = |text: &str| text.replace(' ', "");
    let mut bench = Bench::offered();
    bench.tick(Duration::ZERO);
    bench.paint(Locale::EnUs);
    bench.tick(Duration::ZERO);
    bench.tick(Duration::from_secs(1));
    bench.send(b"\r");
    bench.tick(Duration::ZERO);
    assert!(bench.notice_shown().offered.accepted);

    // Another window wins the claim while the person types the answer they expect to be asked
    // for: the notice says so, and the typing and its Enter reach nothing and close nothing.
    bench.send(b"pass");
    bench.notice(Notice::Taken { login: id(1) });
    bench.tick(Duration::ZERO);
    for _ in 0..4 {
        bench.send(b"word\r");
        bench.tick(Duration::from_millis(600));
    }
    assert!(bench.view.machines.captures());
    assert_eq!(bench.notice_shown().offered.ended, Some(Withdrawn::Taken));
    bench.untouched();
    let english = bench.paint(Locale::EnUs);
    for line in ["Another Orchbee window took this login.", "Close"] {
        assert!(english.contains(&squeezed(line)), "{line}\n{english}");
    }
    let chinese = bench.paint(Locale::ZhCn);
    let line = "另一个 Orchbee 窗口接手了这次登录。";
    assert!(chinese.contains(&squeezed(line)), "{line}\n{chinese}");

    // A second with no key dropped arms it, and Enter closes it: nothing holds the keys.
    bench.tick(Duration::from_secs(1));
    bench.send(b"\r");
    bench.tick(Duration::ZERO);
    assert!(!bench.view.machines.waiting_shown());
    assert!(!bench.view.machines.captures());
    bench.untouched();
}
