//! A scripted run of a login answer through the real input path: a test word typed key by key, as
//! one packet, and pasted, then the frames and traces that run leaves. No real secret exists here.

use super::*;
use crate::client_view::ClientView;
use crate::localization::Locale;
use crate::machine_login::{Effect, Logins, Notice};
use crate::terminal_input::{InputDispatcher, TerminalInputPort, TerminalInputUnavailable};
use crate::tui_hit_map::HitMapBuilder;
use crate::tui_theme::{GlyphSet, Theme};
use orchbee_machine_protocol::{
    AnswerToken, Id, PathText, Prompt,
    hub::{AnswerEndpoint, Question},
};
use std::time::Instant;

/// The test word every assertion looks for.
const WORD: &str = "Mxq-test-word";
const MACHINE: &str = "0123456789abcdef0123456789abcdef";
const TARGET: &str = "user@devbox";
const PASSWORD: &str = "user@devbox's password: ";

/// The terminal port: every byte that would reach a pane.
#[derive(Default)]
struct Port {
    forwarded: Vec<u8>,
}

impl TerminalInputPort for Port {
    fn forward(&mut self, input: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(input);
        Ok(())
    }
}

fn episode() -> Logins {
    let id = |value| Id::new(value).unwrap();
    let mut logins = Logins::default();
    let target = |_: &str| Some(TARGET.to_owned());
    logins.notice(
        Notice::Claimed {
            login: id(1),
            machine: MACHINE.into(),
        },
        target,
    );
    logins.notice(
        Notice::Question(Question {
            login: id(1),
            question: id(1),
            kind: crate::machine_question::classify(PASSWORD),
            prompt: Prompt::new(PASSWORD).unwrap(),
            repeated: false,
            remembered_rejected: false,
            answer: AnswerEndpoint {
                endpoint: PathText::new("answer").unwrap(),
                token: AnswerToken::from_bytes([7; 16]),
            },
        }),
        target,
    );
    logins
}

fn dispatch(
    dispatcher: &mut InputDispatcher,
    view: &mut ClientView,
    port: &mut Port,
    bytes: &[u8],
) {
    let outcome = crate::terminal_input::dispatch_input(
        dispatcher,
        view,
        bytes,
        port,
        Instant::now(),
        |_view, _port| Ok::<(), std::convert::Infallible>(()),
    );
    assert!(outcome.is_ok());
}

/// The frame's text, row by row.
fn paint(picker: &Picker, locale: Locale, theme: &Theme) -> String {
    let backend = ratatui::backend::TestBackend::new(100, 30);
    let mut terminal = ratatui::Terminal::new(backend).unwrap();
    let mut hits = HitMapBuilder::new();
    terminal
        .draw(|frame| picker.render(frame, locale, theme, &mut hits))
        .unwrap();
    let buffer = terminal.backend().buffer();
    (0..30)
        .map(|row| {
            (0..100)
                .map(|column| buffer[(column, row)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn a_typed_or_pasted_answer_reaches_only_the_field() {
    let mut logins = episode();
    let mut view = ClientView::new(100);
    view.machines.login = Some(LoginOverlay::new(logins.presented().unwrap()));
    let mut dispatcher = InputDispatcher::default();
    let mut port = Port::default();
    for byte in WORD.bytes() {
        dispatch(&mut dispatcher, &mut view, &mut port, &[byte]);
    }
    dispatch(&mut dispatcher, &mut view, &mut port, WORD.as_bytes());
    let pasted = [b"\x1b[200~", WORD.as_bytes(), b"\x1b[201~"].concat();
    dispatch(&mut dispatcher, &mut view, &mut port, &pasted);
    let typed = 3 * WORD.chars().count();
    let login = view.machines.login().expect("the question stays open");
    assert_eq!(login.presented.field.characters(), typed);

    // Nothing reached a pane, and no debug form of the view holds the word.
    assert!(port.forwarded.is_empty());
    assert!(!format!("{:?}", view.machines).contains(WORD));
    assert!(!format!("{view:?}").contains(WORD));

    // Every frame shows one mask per character and nothing of the word.
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
            let theme = Theme::terminal_native().with_glyphs(glyphs);
            let frame = paint(&view.machines, locale, &theme);
            assert!(!frame.contains(WORD), "{locale:?} {glyphs:?}\n{frame}");
            assert!(
                frame.contains(&glyphs.secret_mask.repeat(typed)),
                "{locale:?} {glyphs:?}\n{frame}"
            );
        }
    }

    // Enter hands the whole answer to the episode at once; later keys do not join it.
    dispatch(&mut dispatcher, &mut view, &mut port, b"\rlate");
    let (token, act) = view.machines.login.as_mut().unwrap().act.take().unwrap();
    assert_eq!(act, Act::Submit);
    let effects = logins.act(token, act);
    let [Effect::Deliver { answer, .. }] = effects.as_slice() else {
        panic!("one delivery");
    };
    assert_eq!(answer.as_bytes(), WORD.repeat(3).as_bytes());
    assert!(port.forwarded.is_empty());
}

#[test]
fn esc_cancels_the_login_and_its_selection() {
    let logins = episode();
    let mut view = ClientView::new(100);
    // The question arrived while the picker that started the selection was open.
    view.machines.show();
    view.machines.login = Some(LoginOverlay::new(logins.presented().unwrap()));
    let mut dispatcher = InputDispatcher::default();
    let mut port = Port::default();
    // The unambiguous Escape form; a lone ESC waits for the decoder's timeout.
    dispatch(&mut dispatcher, &mut view, &mut port, b"\x1b[27u");
    let login = view.machines.login().unwrap();
    assert!(matches!(login.act, Some((_, Act::Cancel))));
    assert!(port.forwarded.is_empty());
    // Once the episode is gone nothing unseen holds the keys: the person is where they were.
    view.machines.login = None;
    assert!(!view.machines.captures());
}
