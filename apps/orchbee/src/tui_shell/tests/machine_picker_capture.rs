//! Composed Machine Picker evidence from the real shell frame and central input path.

use super::*;
use crate::{
    client_view::{KeyCode, Modifiers},
    machine_picker::Choice,
    terminal_input::InputDispatcher,
    tui_theme::Theme,
    workbench_client::ClientMemberTarget,
};
use orchbee_workbench_client::test_support::client_view::KeyEvent;
use orchbee_workbench_client::test_support::localization::LocaleTestAccess;
use orchbee_workbench_client::test_support::machine_agents::Observation as AgentObservation;
use orchbee_workbench_client::test_support::machine_agents::{checkpoint, row};
use orchbee_workbench_client::test_support::machine_picker::Control;
use orchbee_workbench_client::test_support::machine_picker::DrawnPickerTargetTestAccess;
use orchbee_workbench_client::test_support::machine_picker::Mode;
use orchbee_workbench_client::test_support::machine_picker::PickerAction;
use orchbee_workbench_client::test_support::machine_picker::PickerSourceTestAccess;
use orchbee_workbench_client::test_support::machine_picker::PickerTestAccess;
use orchbee_workbench_client::test_support::terminal_events::keys::KeyEventTestAccess;

#[derive(Default)]
struct Surface {
    forwarded: Vec<u8>,
}

impl TerminalInputPort for Surface {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
}

impl super::super::TerminalSurface for Surface {}

fn screen_text(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    buffer
        .content()
        .chunks(usize::from(buffer.area.width))
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

fn screen_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    (buffer.area.y..buffer.area.bottom())
        .map(|row| {
            (buffer.area.x..buffer.area.right())
                .map(|column| buffer[(column, row)].symbol())
                .collect()
        })
        .collect()
}

fn without_cell_padding(value: &str) -> String {
    value.replace(' ', "")
}

fn expected_too_small_notice(locale: Locale, width: u16, theme: &Theme) -> String {
    let marker = format!("{} ", theme.glyphs.status_bullet);
    let marker_width =
        orchbee_workbench_client::test_support::terminal_text::display_width(&marker);
    if marker_width < usize::from(width) {
        format!(
            "{marker}{}",
            orchbee_workbench_client::test_support::terminal_text::truncate_display(
                locale.shell().too_small_notice,
                usize::from(width) - marker_width,
                theme.glyphs.ellipsis,
            )
        )
    } else {
        orchbee_workbench_client::test_support::terminal_text::truncate_display(
            locale.shell().too_small_notice,
            usize::from(width),
            theme.glyphs.ellipsis,
        )
    }
}

fn find_ascii(terminal: &Terminal<TestBackend>, needle: &str) -> (u16, u16) {
    let expected = needle.chars().map(|ch| ch.to_string()).collect::<Vec<_>>();
    let width = u16::try_from(expected.len()).unwrap();
    let buffer = terminal.backend().buffer();
    for row in buffer.area.y..buffer.area.bottom() {
        for column in buffer.area.x..=buffer.area.right().saturating_sub(width) {
            if expected.iter().enumerate().all(|(offset, symbol)| {
                buffer[(column + u16::try_from(offset).unwrap(), row)].symbol() == symbol
            }) {
                return (column, row);
            }
        }
    }
    panic!("missing painted text {needle:?}");
}

fn draw(
    terminal: &mut Terminal<TestBackend>,
    shell: &mut ClientView,
    surface: &mut Surface,
    dispatcher: &mut InputDispatcher,
    theme: &Theme,
) {
    draw_locale(terminal, shell, surface, dispatcher, Locale::EnUs, theme);
}

fn draw_locale(
    terminal: &mut Terminal<TestBackend>,
    shell: &mut ClientView,
    surface: &mut Surface,
    dispatcher: &mut InputDispatcher,
    locale: Locale,
    theme: &Theme,
) {
    super::super::draw_surface(
        terminal,
        shell,
        super::super::Chrome { locale, theme },
        surface,
        dispatcher,
        &mut |_| Ok(()),
    )
    .unwrap();
}

fn dispatch_pointer(
    dispatcher: &mut InputDispatcher,
    shell: &mut ClientView,
    surface: &mut Surface,
    cell: (u16, u16),
    release: bool,
) {
    let report = format!(
        "\x1b[<0;{};{}{}",
        cell.0 + 1,
        cell.1 + 1,
        if release { 'm' } else { 'M' }
    );
    dispatcher
        .dispatch(
            shell,
            report.as_bytes(),
            surface,
            Instant::now(),
            |_, _| Ok::<bool, io::Error>(false),
            &mut |_| Ok(()),
        )
        .unwrap();
}

#[test]
fn machine_picker_root_frame_captures_real_agents_and_activates_search_on_exact_release() {
    const WIDTH: u16 = 120;
    const HEIGHT: u16 = 40;

    let current = row(23);
    let provisional = row(24);
    let focus = current.target.clone();
    let mut shell = ClientView::new(WIDTH);
    shell.machines.available = true;
    assert!(shell.machines.observe_agents(vec![(
        Choice::Local,
        AgentObservation::Ready(checkpoint(vec![current, provisional])),
    )]));
    assert!(shell.machines.observe_focus(
        Choice::Local,
        Some(focus.context.clone()),
        Some(ClientMemberTarget {
            tab: focus.tab.clone(),
            panel: focus.panel.clone(),
            presentation: focus.member.clone(),
        }),
    ));
    shell.machines.show_agents();
    shell
        .machines
        .key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));

    let mut terminal = Terminal::new(TestBackend::new(WIDTH, HEIGHT)).unwrap();
    let mut surface = Surface::default();
    let mut dispatcher = InputDispatcher::default();
    dispatcher.set_pointer_capture(&mut shell, &mut surface, true);
    dispatcher
        .synchronize(&mut shell, &mut surface, Instant::now(), &mut |_| {
            Ok::<(), io::Error>(())
        })
        .unwrap();
    let theme = Theme::terminal_native();
    draw(
        &mut terminal,
        &mut shell,
        &mut surface,
        &mut dispatcher,
        &theme,
    );

    let text = screen_text(&terminal);
    for visible in [
        "Agents 2/2",
        "Machines",
        "Workspaces",
        "All states",
        "Placement",
        "All machines",
        "/ to search agents",
        "Clear",
        "Esc",
        "Close",
        "Enter",
        "Open",
        "Tab",
        "Mode",
    ] {
        assert!(text.contains(visible), "missing {visible:?} in {text:?}");
    }

    // A local-only list omits the machine prefix; a distinct task title follows the name.
    let first = find_ascii(&terminal, "1 codex · build");
    let second = find_ascii(&terminal, "2 codex · build");
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(first.0 - 2, first.1)].symbol(),
        theme.glyphs.active_marker,
        "the first real Agent remains current"
    );
    assert_eq!(
        buffer[(second.0 - 1, second.1)].symbol(),
        theme.glyphs.navigation_marker,
        "the second real Agent is the distinct provisional row"
    );

    let source = shell.machines.picker_source().unwrap();
    for action in [
        PickerAction::Close,
        PickerAction::Mode(Mode::Machines),
        PickerAction::Mode(Mode::Workspaces),
        PickerAction::Mode(Mode::Agents),
        PickerAction::Control(Control::State),
        PickerAction::Control(Control::Order),
        PickerAction::Control(Control::Scope),
        PickerAction::Control(Control::Search),
        PickerAction::Control(Control::Clear),
        PickerAction::Row(0),
        PickerAction::Row(1),
    ] {
        assert!(
            source
                .drawn(action)
                .is_some_and(|target| target.available()),
            "the composed frame source must expose {action:?} as a typed Machine control"
        );
    }
    assert!(
        source.drawn(PickerAction::Control(Control::Done)).is_none(),
        "the raw Agent query has not entered editing mode"
    );

    orchbee_workbench_client::test_support::tui_render::test_capture::capture_screen(
        &mut terminal,
        "alignment-machine-picker-en-us-120x40-terminal-native",
        Locale::EnUs,
        &theme,
    );

    let search = find_ascii(&terminal, "/ to search agents");
    dispatch_pointer(&mut dispatcher, &mut shell, &mut surface, search, false);
    assert_eq!(
        shell.machines.picker_source(),
        Some(source.clone()),
        "press only arms the successfully published typed target"
    );
    assert!(shell.machines.request.is_none());

    draw(
        &mut terminal,
        &mut shell,
        &mut surface,
        &mut dispatcher,
        &theme,
    );
    dispatch_pointer(&mut dispatcher, &mut shell, &mut surface, search, true);
    let editing = shell.machines.picker_source().unwrap();
    assert_ne!(editing, source);
    assert!(
        editing
            .drawn(PickerAction::Control(Control::Done))
            .is_some_and(|target| target.available()),
        "the matching release alone activates Search"
    );
    assert!(shell.machines.request.is_none());
    assert!(surface.forwarded.is_empty());
}

#[test]
fn too_small_machine_frames_preserve_the_root_notice_and_only_keyboard_escape_closes() {
    let theme = Theme::terminal_native();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for (width, height) in [(100, 1), (100, 3), (100, 4), (19, 24)] {
            let current = row(23);
            let provisional = row(24);
            let focus = current.target.clone();
            let mut shell = ClientView::new(width);
            shell.machines.available = true;
            assert!(shell.machines.observe_agents(vec![(
                Choice::Local,
                AgentObservation::Ready(checkpoint(vec![current, provisional])),
            )]));
            assert!(shell.machines.observe_focus(
                Choice::Local,
                Some(focus.context.clone()),
                Some(ClientMemberTarget {
                    tab: focus.tab.clone(),
                    panel: focus.panel.clone(),
                    presentation: focus.member.clone(),
                }),
            ));
            shell.machines.show_agents();
            shell
                .machines
                .key(KeyEvent::press(KeyCode::Down, Modifiers::NONE));
            let source = shell.machines.picker_source().unwrap();

            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut surface = Surface::default();
            let mut dispatcher = InputDispatcher::default();
            dispatcher.set_pointer_capture(&mut shell, &mut surface, true);
            dispatcher
                .synchronize(&mut shell, &mut surface, Instant::now(), &mut |_| {
                    Ok::<(), io::Error>(())
                })
                .unwrap();
            draw_locale(
                &mut terminal,
                &mut shell,
                &mut surface,
                &mut dispatcher,
                locale,
                &theme,
            );

            assert!(shell.machines.open, "{locale:?} {width}x{height}");
            assert_eq!(
                shell.machines.picker_source(),
                Some(source.clone()),
                "{locale:?} {width}x{height} paint changed the live Machine source"
            );
            let rows = screen_rows(&terminal);
            let visible_body = rows[..usize::from(height.saturating_sub(1))]
                .iter()
                .map(|row| without_cell_padding(row))
                .filter(|row| !row.is_empty())
                .collect::<Vec<_>>();
            let expected_body = (height > 1)
                .then(|| without_cell_padding(&expected_too_small_notice(locale, width, &theme)))
                .into_iter()
                .collect::<Vec<_>>();
            assert_eq!(
                visible_body, expected_body,
                "{locale:?} {width}x{height} painted a Machine panel over the root notice"
            );

            let status = &rows[usize::from(height - 1)];
            let close_hint = format!("Esc {}", locale.machine_picker_text("picker-close-hint"));
            assert!(
                without_cell_padding(status).contains(&without_cell_padding(&close_hint)),
                "{locale:?} {width}x{height} lost its complete local close hint: {status:?}"
            );
            assert!(
                !status.contains("Ctrl+b q"),
                "{locale:?} {width}x{height} retained the covered root status: {status:?}"
            );
            if width == 19 {
                let ordered_hints = [
                    close_hint.clone(),
                    format!("↑/↓ {}", locale.machine_picker_text("picker-choose-hint")),
                    format!("Enter {}", locale.machine_picker_text("picker-open-hint")),
                    format!("Tab {}", locale.machine_picker_text("picker-mode-hint")),
                ];
                let separator = format!(" {} ", theme.glyphs.middle_dot);
                let mut expected_status = String::new();
                for hint in ordered_hints {
                    let candidate = if expected_status.is_empty() {
                        hint
                    } else {
                        format!("{expected_status}{separator}{hint}")
                    };
                    if orchbee_workbench_client::test_support::terminal_text::display_width(
                        &candidate,
                    ) > usize::from(width)
                    {
                        break;
                    }
                    expected_status = candidate;
                }
                assert_eq!(
                    without_cell_padding(status),
                    without_cell_padding(&expected_status),
                    "the narrow status must retain the longest ordered prefix of whole localized hints"
                );
                assert!(
                    orchbee_workbench_client::test_support::terminal_text::display_width(
                        &expected_status
                    ) <= usize::from(width),
                    "the complete localized hint prefix must be width-safe"
                );
            }

            let escape_cell = find_ascii(&terminal, "Esc");
            dispatch_pointer(
                &mut dispatcher,
                &mut shell,
                &mut surface,
                escape_cell,
                false,
            );
            draw_locale(
                &mut terminal,
                &mut shell,
                &mut surface,
                &mut dispatcher,
                locale,
                &theme,
            );
            dispatch_pointer(&mut dispatcher, &mut shell, &mut surface, escape_cell, true);
            assert!(
                shell.machines.open,
                "{locale:?} {width}x{height} published a mouse action for status-only paint"
            );
            assert_eq!(shell.machines.picker_source(), Some(source));
            assert!(shell.machines.request.is_none());

            let escape_at = Instant::now();
            dispatcher
                .dispatch(
                    &mut shell,
                    b"\x1b",
                    &mut surface,
                    escape_at,
                    |_, _| Ok::<bool, io::Error>(false),
                    &mut |_| Ok(()),
                )
                .unwrap();
            assert!(shell.machines.open, "a lone Esc awaits disambiguation");
            dispatcher
                .synchronize(
                    &mut shell,
                    &mut surface,
                    escape_at + std::time::Duration::from_secs(1),
                    &mut |_| Ok::<(), io::Error>(()),
                )
                .unwrap();
            assert!(
                !shell.machines.open,
                "{locale:?} {width}x{height} keyboard Esc did not close the retained model"
            );
            assert!(shell.machines.picker_source().is_none());
            assert!(shell.machines.selection_scope().is_none());
            assert!(surface.forwarded.is_empty());
        }
    }
}
