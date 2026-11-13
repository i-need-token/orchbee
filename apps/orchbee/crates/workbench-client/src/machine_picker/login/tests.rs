//! Scripted runs of a login overlay through the real input path: a test word typed key by key, as
//! one packet, and pasted; a login that ends while the person types; and windows too small for the
//! question. No real secret exists here.

use super::*;
use crate::client_view::{ClientActionDisposition, ClientView};
use crate::localization::{Locale, MachineQuestion};
use crate::machine_login::{Effect, Logins, Notice};
use crate::terminal_input::{
    CopyAction, InputDispatcher, MenuAction, PaneAdmission, PaneInput, SurfaceAction, SurfaceInput,
    TerminalInputPort, TerminalInputUnavailable,
};
use crate::tui_hit_map::HitMapBuilder;
use crate::tui_theme::{GlyphSet, Theme};
use crate::workbench_client::input::InputAction;
use orchbee_machine_protocol::{
    AnswerToken, Id, PathText, Prompt,
    hub::{AnswerEndpoint, LoginEnd, Question},
};
use std::time::Instant;

/// The test word every assertion looks for.
const WORD: &str = "Mxq-test-word";
const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const OTHER_MACHINE: &str = "fedcba9876543210fedcba9876543210";
const TARGET: &str = "user@devbox";
const PASSWORD: &str = "user@devbox's password: ";
const NEW_HOST: &str = "The authenticity of host 'devbox (10.0.0.8)' can't be established.\n\
ED25519 key fingerprint is SHA256:Qx7mT2vLk9PzR4wN8cYhB3jF6sD1aG5eU0iO2pK7lMn.\n\
Are you sure you want to continue connecting (yes/no/[fingerprint])? ";

/// A server's question only `ssh` words: a notice longer than the overlay keeps, then the line
/// that asks.
fn notice_then_code() -> String {
    let mut prompt: String = (1..=20)
        .map(|line| format!("Notice line {line} of the server's message.\n"))
        .collect();
    prompt.push_str("Verification code: ");
    prompt
}

/// The terminal port: counts every input that would reach a pane, a surface or an action.
#[derive(Default)]
struct Port {
    forwarded: Vec<u8>,
    /// The port calls an input reached, by name.
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

fn question(login: u32, prompt: &str) -> Notice {
    Notice::Question(Question {
        login: id(login),
        question: id(1),
        kind: crate::machine_question::classify(prompt),
        prompt: Prompt::new(prompt).unwrap(),
        repeated: false,
        remembered_rejected: false,
        answer: AnswerEndpoint {
            endpoint: PathText::new("answer").unwrap(),
            token: AnswerToken::from_bytes([7; 16]),
        },
    })
}

fn claimed(login: u32, machine: &str) -> Notice {
    Notice::Claimed {
        login: id(login),
        machine: machine.into(),
    }
}

/// Plays one hub notice into `logins` for Machines whose `ssh` target is `TARGET`.
fn play(logins: &mut Logins, notice: Notice) -> Vec<Effect> {
    logins.notice(notice, |machine| {
        [MACHINE, OTHER_MACHINE]
            .contains(&machine)
            .then(|| TARGET.to_owned())
    })
}

/// What the overlay starts over.
#[derive(Clone, Copy, Debug)]
enum Under {
    Picker,
    WorkspaceQuery,
    Nothing,
}

/// A login that ends without the person.
#[derive(Clone, Copy, Debug)]
enum End {
    Ended,
    Lost,
}

/// One view over the real dispatcher, and the login episodes the shell carries to it.
struct Bench {
    view: ClientView,
    dispatcher: InputDispatcher,
    port: Port,
    discovery: Discovery,
}

impl Bench {
    /// The person waits on `MACHINE`, whose login asks `prompt`.
    fn asked(under: Under, prompt: &str) -> Self {
        let mut view = ClientView::new(100);
        view.machines.available = true;
        match under {
            Under::Picker => view.machines.show(),
            Under::WorkspaceQuery => view.machines.show_workspaces(),
            Under::Nothing => {}
        }
        let mut bench = Self {
            view,
            dispatcher: InputDispatcher::default(),
            port: Port::default(),
            discovery: Discovery::default(),
        };
        let logins = bench.discovery.logins_mut();
        assert!(logins.wait_for(Some(MACHINE)).is_empty());
        assert!(play(logins, claimed(1, MACHINE)).is_empty());
        assert!(play(logins, question(1, prompt)).is_empty());
        bench.exchange();
        assert!(
            bench.view.machines.login().is_some(),
            "the question is shown"
        );
        bench
    }

    fn notice(&mut self, notice: Notice) -> Vec<Effect> {
        let effects = play(self.discovery.logins_mut(), notice);
        self.exchange();
        effects
    }

    /// What the shell does after input: carries the act to the episodes, then shows what they
    /// present.
    fn exchange(&mut self) {
        self.view.machines.exchange_login(&mut self.discovery);
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
    }

    fn login(&self) -> &LoginOverlay {
        self.view.machines.login().expect("the overlay is shown")
    }

    fn paint(&mut self, width: u16, height: u16, locale: Locale, theme: &Theme) -> String {
        paint(&mut self.view.machines, width, height, locale, theme)
    }
}

/// The frame's text, row by row; painting records whether the whole question was shown.
fn paint(picker: &mut Picker, width: u16, height: u16, locale: Locale, theme: &Theme) -> String {
    let backend = ratatui::backend::TestBackend::new(width, height);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| picker.render(frame, locale, theme, &mut hits))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..height)
        .map(|row| {
            (0..width)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `text` with its spaces and frame glyphs gone, so wrapped text reads as one run.
fn squeezed(text: &str) -> String {
    text.chars()
        .filter(|character| {
            !character.is_whitespace() && !('\u{2500}'..='\u{257f}').contains(character)
        })
        .collect()
}

/// The picker as the person left it, without the overlay over it.
fn beneath(picker: &Picker) -> Picker {
    let mut picker = picker.clone();
    picker.forget_login();
    picker
}

#[test]
fn a_typed_or_pasted_answer_reaches_only_the_field() {
    let mut logins = Logins::default();
    assert!(logins.wait_for(Some(MACHINE)).is_empty());
    play(&mut logins, claimed(1, MACHINE));
    play(&mut logins, question(1, PASSWORD));
    let mut bench = Bench {
        view: ClientView::new(100),
        dispatcher: InputDispatcher::default(),
        port: Port::default(),
        discovery: Discovery::default(),
    };
    bench.view.machines.login = Some(LoginOverlay::new(logins.presented().unwrap()));
    let theme = Theme::terminal_native();
    bench.paint(100, 30, Locale::EnUs, &theme);
    for byte in WORD.bytes() {
        bench.send(&[byte]);
    }
    bench.send(WORD.as_bytes());
    let pasted = [b"\x1b[200~", WORD.as_bytes(), b"\x1b[201~"].concat();
    bench.send(&pasted);
    let typed = 3 * WORD.chars().count();
    assert_eq!(bench.login().presented.field.characters(), typed);

    // Nothing reached a pane, and no debug form of the view holds the word.
    assert!(bench.port.forwarded.is_empty());
    assert_eq!(bench.port.reached, [""; 0]);
    assert!(!format!("{:?}", bench.view.machines).contains(WORD));
    assert!(!format!("{:?}", bench.view).contains(WORD));

    // Every frame shows one mask per character and nothing of the word.
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
            let theme = Theme::terminal_native().with_glyphs(glyphs);
            let frame = bench.paint(100, 30, locale, &theme);
            assert!(!frame.contains(WORD), "{locale:?} {glyphs:?}\n{frame}");
            assert!(
                frame.contains(&glyphs.secret_mask.repeat(typed)),
                "{locale:?} {glyphs:?}\n{frame}"
            );
        }
    }

    // Enter hands the whole answer to the episode at once; later keys do not join it.
    bench.send(b"\rlate");
    let (token, act) = bench
        .view
        .machines
        .login
        .as_mut()
        .unwrap()
        .act
        .take()
        .unwrap();
    assert_eq!(act, Act::Submit);
    let effects = logins.act(token, act);
    let [Effect::Deliver { answer, .. }] = effects.as_slice() else {
        panic!("one delivery");
    };
    assert_eq!(answer.as_bytes(), WORD.repeat(3).as_bytes());
    assert!(bench.port.forwarded.is_empty());
}

#[test]
fn a_login_that_ends_while_the_person_types_leaves_no_trace_of_the_word() {
    let mut bench = Bench::asked(Under::Nothing, PASSWORD);
    let theme = Theme::terminal_native();
    bench.paint(100, 30, Locale::EnUs, &theme);
    let (before, after) = WORD.split_at(WORD.len() / 2);
    bench.send(before.as_bytes());
    bench.notice(Notice::Ended {
        login: id(1),
        end: LoginEnd::Failed,
    });
    bench.send(after.as_bytes());
    bench.send(&[b"\x1b[200~", WORD.as_bytes(), b"\x1b[201~"].concat());
    assert!(bench.login().ended());
    assert_eq!(bench.login().presented.field.characters(), 0);
    assert!(bench.port.forwarded.is_empty());
    assert_eq!(bench.port.reached, [""; 0]);
    assert!(!format!("{:?}", bench.view).contains(before));
    assert!(!format!("{:?}", bench.view).contains(after));
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
            let theme = Theme::terminal_native().with_glyphs(glyphs);
            let frame = bench.paint(100, 30, locale, &theme);
            assert!(!frame.contains(before), "{locale:?} {glyphs:?}\n{frame}");
            assert!(!frame.contains(after), "{locale:?} {glyphs:?}\n{frame}");
            // The mask glyph is also the status bullet: a field shows as a run of masks.
            assert!(
                !frame.contains(&glyphs.secret_mask.repeat(2)),
                "{locale:?} {glyphs:?}\n{frame}"
            );
        }
    }
}

#[test]
fn after_a_login_ends_by_itself_only_esc_or_enter_reach_its_overlay() {
    /// Keys and a paste the person may press before reading the outcome; none is Esc or Enter.
    const AFTER: &[&[u8]] = &[
        b"rest",
        b"j",
        b"k",
        b" ",
        b"/",
        b"q",
        b"\t",
        b"\x7f",
        b"\x15",
        b"\x02",
        b"\x1b[A",
        b"\x1b[B",
        b"\x1b[C",
        b"\x1b[1;5D",
        b"\x1bx",
        b"\x1b[105;5u",
        b"\x1b[200~pasted\x1b[201~",
    ];
    let theme = Theme::terminal_native();
    for under in [Under::Picker, Under::WorkspaceQuery, Under::Nothing] {
        for end in [End::Ended, End::Lost] {
            for close in [&b"\r"[..], b"\x1b[27u"] {
                let case = format!("{under:?} {end:?} {close:?}");
                let mut bench = Bench::asked(under, PASSWORD);
                bench.paint(100, 30, Locale::EnUs, &theme);
                bench.send(b"half");
                assert_eq!(bench.login().presented.field.characters(), 4, "{case}");
                bench.notice(match end {
                    End::Ended => Notice::Ended {
                        login: id(1),
                        end: LoginEnd::Failed,
                    },
                    End::Lost => Notice::Lost,
                });
                assert!(bench.login().ended(), "{case}");
                let left = beneath(&bench.view.machines);
                let frame = bench.paint(100, 30, Locale::EnUs, &theme);
                assert!(
                    !frame.contains(&theme.glyphs.secret_mask.repeat(2)),
                    "{case}\n{frame}"
                );
                for keys in AFTER {
                    bench.send(keys);
                    bench.exchange();
                    assert!(bench.login().ended(), "{case} {keys:?}");
                    assert_eq!(bench.login().presented.field.characters(), 0, "{case}");
                }
                // No picker, query field, pane or action saw a key, and no login started.
                assert_eq!(beneath(&bench.view.machines), left, "{case}");
                assert!(bench.view.machines.request.is_none(), "{case}");
                assert!(bench.port.forwarded.is_empty(), "{case}");
                assert_eq!(bench.port.reached, [""; 0], "{case}");
                assert!(bench.discovery.login_presented().is_some(), "{case}");

                // Esc or Enter closes it, onto the state the end left.
                bench.send(close);
                bench.exchange();
                assert!(bench.view.machines.login().is_none(), "{case}");
                assert!(bench.discovery.login_presented().is_none(), "{case}");
                assert_eq!(bench.view.machines, left, "{case}");
                if matches!(under, Under::WorkspaceQuery) {
                    // The query was live underneath all along: it takes keys again now.
                    bench.send(b"z");
                    assert_eq!(bench.view.machines.workspace_query.text(), "z", "{case}");
                }
            }
        }
    }
}

#[test]
fn only_the_awaited_login_is_shown_and_a_shown_one_is_never_replaced() {
    // A selection let go before its login asked: the login is dismissed and nothing shows.
    let mut bench = Bench {
        view: ClientView::new(100),
        dispatcher: InputDispatcher::default(),
        port: Port::default(),
        discovery: Discovery::default(),
    };
    let logins = bench.discovery.logins_mut();
    assert!(logins.wait_for(Some(MACHINE)).is_empty());
    assert!(logins.wait_for(None).is_empty());
    let effects = bench.notice(claimed(1, MACHINE));
    assert!(
        matches!(effects.as_slice(), [Effect::Dismiss(login)] if *login == id(1)),
        "dismissed on arrival: {effects:?}"
    );
    assert!(bench.notice(question(1, PASSWORD)).is_empty());
    assert!(bench.view.machines.login().is_none());
    assert!(!bench.view.machines.captures());

    // Another Machine's login while one overlay is open is dismissed; the open one is untouched.
    let mut bench = Bench::asked(Under::Picker, PASSWORD);
    let theme = Theme::terminal_native();
    bench.paint(100, 30, Locale::EnUs, &theme);
    bench.send(b"abc");
    let shown = bench.login().presented.token;
    let effects = bench.notice(claimed(2, OTHER_MACHINE));
    assert!(
        matches!(effects.as_slice(), [Effect::Dismiss(login)] if *login == id(2)),
        "{effects:?}"
    );
    assert!(bench.notice(question(2, PASSWORD)).is_empty());
    assert_eq!(bench.login().presented.token, shown);
    assert_eq!(bench.login().presented.field.characters(), 3);
    bench.send(b"d");
    assert_eq!(bench.login().presented.field.characters(), 4);
}

/// Whether `frame` shows the sentence `presented` asks with: Orchbee's for a question it words,
/// `ssh`'s last line for one only `ssh` words, and a new host's whole fingerprint; and whether a
/// cut of `ssh`'s lines is marked.
fn asks_on_screen(frame: &str, presented: &Presented, locale: Locale) -> bool {
    let frame = squeezed(frame);
    let question = MachineQuestion::of(&presented.kind);
    let said = locale.machine_question_said();
    let lines = crate::machine_question::shown_lines(presented.prompt.as_str());
    let last = lines.last().map(|line| squeezed(&format!("{said}{line}")));
    let first = lines.first().map(|line| squeezed(&format!("{said}{line}")));
    let cut = squeezed(&format!("{said}{}", crate::machine_question::CUT));
    // A frame that lost the first line but shows a later one marks the cut.
    let marked = match (&first, &last) {
        (Some(first), Some(last)) if frame.contains(last.as_str()) => {
            frame.contains(first.as_str()) || frame.contains(cut.as_str())
        }
        _ => true,
    };
    let asked = match &question {
        MachineQuestion::HostKey {
            algorithm,
            fingerprint,
        } => [
            locale.machine_question_new_host_lead(&presented.target, algorithm),
            (*fingerprint).to_owned(),
            locale.machine_question_type_yes().to_owned(),
        ]
        .iter()
        .all(|text| frame.contains(&squeezed(text))),
        MachineQuestion::Other => last.is_some_and(|last| frame.contains(&last)),
        MachineQuestion::Password | MachineQuestion::KeyPassphrase => frame.contains(&squeezed(
            &locale.machine_question_asked(&presented.target, &question, false),
        )),
    };
    asked && marked
}

#[test]
fn an_answer_is_submitted_only_while_its_question_is_on_screen() {
    let theme = Theme::terminal_native();
    let other = notice_then_code();
    for (prompt, answer) in [
        (PASSWORD, &b"abc"[..]),
        (NEW_HOST, b"yes"),
        (other.as_str(), b"123456"),
    ] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let mut submitted = 0;
            let mut refused = 0;
            for width in [20, 24, 30, 36, 40, 48, 56, 60, 64, 72, 80, 100] {
                for height in [5, 6, 8, 10, 12, 14, 16, 18, 20, 24, 30, 40] {
                    let case = format!("{prompt:?} {locale:?} {width}x{height}");
                    let mut bench = Bench::asked(Under::Nothing, prompt);
                    let frame = bench.paint(width, height, locale, &theme);
                    bench.send(answer);
                    bench.send(b"\r");
                    let login = bench.login();
                    if matches!(login.act, Some((_, Act::Submit | Act::Trust))) {
                        submitted += 1;
                        assert!(
                            asks_on_screen(&frame, &login.presented, locale),
                            "{case}\n{frame}"
                        );
                    } else {
                        refused += 1;
                        // Off screen, the typed answer went nowhere and no act waits.
                        assert!(login.act.is_none(), "{case}");
                        assert_eq!(login.presented.field.characters(), 0, "{case}");
                    }
                    assert!(bench.port.forwarded.is_empty(), "{case}");
                }
            }
            assert!(submitted > 0 && refused > 0, "{prompt:?} {locale:?}");
        }
    }
}

#[test]
fn a_window_too_small_for_the_question_keeps_the_typed_field_for_when_it_grows() {
    let theme = Theme::terminal_native();
    let mut bench = Bench::asked(Under::Nothing, PASSWORD);
    bench.paint(100, 30, Locale::EnUs, &theme);
    bench.send(b"ab");
    // A resize leaves the question unshown until a frame shows it whole again.
    bench.view.machines.login_undrawn();
    bench.send(b"cd\r");
    assert!(bench.login().act.is_none());
    let frame = bench.paint(24, 6, Locale::EnUs, &theme);
    assert!(
        !frame.contains(&theme.glyphs.secret_mask.repeat(2)),
        "{frame}"
    );
    bench.send(b"ef\r");
    assert!(bench.login().act.is_none());
    assert_eq!(bench.login().presented.field.characters(), 2);
    let frame = bench.paint(100, 30, Locale::EnUs, &theme);
    assert!(
        frame.contains(&theme.glyphs.secret_mask.repeat(2)),
        "{frame}"
    );
    bench.send(b"\r");
    assert!(matches!(bench.login().act, Some((_, Act::Submit))));
}

#[test]
fn esc_cancels_the_login_and_its_selection() {
    let mut bench = Bench::asked(Under::Picker, PASSWORD);
    // The unambiguous Escape form; a lone ESC waits for the decoder's timeout. Esc acts even
    // before a frame showed the question.
    bench.send(b"\x1b[27u");
    assert!(matches!(bench.login().act, Some((_, Act::Cancel))));
    assert!(bench.port.forwarded.is_empty());
    // Once the episode is gone nothing unseen holds the keys: the person is where they were.
    bench.exchange();
    assert!(bench.view.machines.login().is_none());
    assert!(!bench.view.machines.captures());
}

#[test]
fn a_new_hosts_field_shows_only_the_start_of_yes_in_clear() {
    let theme = Theme::terminal_native();
    let mut bench = Bench::asked(Under::Nothing, NEW_HOST);
    bench.paint(100, 30, Locale::EnUs, &theme);
    bench.send(b"YE");
    assert_eq!(bench.login().shown_yes().as_deref(), Some("YE"));
    bench.send(b"t");
    assert_eq!(bench.login().shown_yes(), None);
    let frame = bench.paint(100, 30, Locale::EnUs, &theme);
    assert!(!frame.contains("YEt"), "{frame}");
    assert!(
        frame.contains(&theme.glyphs.secret_mask.repeat(3)),
        "{frame}"
    );
    // Only a field that reads `yes` trusts.
    bench.send(b"\r");
    assert!(bench.login().act.is_none());
    assert!(bench.login().type_yes);
    bench.send(b"\x15yes\r");
    assert!(matches!(bench.login().act, Some((_, Act::Trust))));
}

#[test]
fn an_empty_password_is_not_sent() {
    let theme = Theme::terminal_native();
    let mut bench = Bench::asked(Under::Nothing, PASSWORD);
    bench.paint(100, 30, Locale::EnUs, &theme);
    bench.send(b"\r");
    assert!(bench.login().act.is_none());
    bench.send(b"a\r");
    assert!(matches!(bench.login().act, Some((_, Act::Submit))));
}
