//! Root-frame notification admission across a TooSmall-to-presentable resize.

use super::*;
use crate::{
    terminal_input::InputDispatcher,
    tui_theme::Theme,
    workbench_client::{DrawReceipt, WorkbenchClientView},
};
use orchbee_local_runtime::{NotificationDelivery, NotificationSettings};
use orchbee_workbench_client::test_support::localization::LocaleTestAccess;
use orchbee_workbench_client::test_support::localization::ShellTextTestAccess;
use orchbee_workbench_client::test_support::notifications::StateTestAccess;
use orchbee_workbench_client::test_support::tui_theme::GlyphSetTestAccess;
use orchbee_workbench_client::test_support::tui_theme::ThemeTestAccess;
use orchbee_workbench_protocol::{Notification, NotificationPosition, NotificationSound};
use ratatui::layout::Rect;

#[derive(Default)]
struct Surface {
    forwarded: Vec<u8>,
    receipts: Vec<DrawReceipt>,
    successful_frames: usize,
    pane_observations: usize,
}

impl TerminalInputPort for Surface {
    fn forward(&mut self, bytes: &[u8]) -> Result<(), TerminalInputUnavailable> {
        self.forwarded.extend_from_slice(bytes);
        Ok(())
    }
}

impl super::super::TerminalSurface for Surface {
    fn view(&self) -> Option<WorkbenchClientView<'_>> {
        Some(WorkbenchClientView::notification_test_view())
    }

    fn after_close_draw(&mut self, receipt: DrawReceipt) {
        self.receipts.push(receipt);
    }

    fn after_draw(&mut self) {
        self.successful_frames += 1;
    }

    fn observe_panes(
        &mut self,
        _: &[(
            orchbee_workbench_protocol::WorkbenchPanelId,
            usize,
            orchbee_client_server_protocol::Geometry,
        )],
    ) {
        self.pane_observations += 1;
    }
}

#[test]
fn native_too_small_withholds_membership_observations_and_resumes_at_the_minimal_boundary() {
    let projection = crate::tui_layout::ResizeProjection::Canonical;
    let mut surface = Surface::default();
    for (size, expected_reports) in [
        (dimensions(80, 24), 1),
        (dimensions(15, 4), 1),
        (dimensions(15, 10), 1),
        (dimensions(80, 4), 1),
        (dimensions(80, 24), 2),
        (dimensions(20, 5), 3),
        (dimensions(80, 4), 3),
        (dimensions(80, 24), 4),
    ] {
        super::super::observe_surface_geometry(
            &size,
            &ClientView::new(size.columns),
            &mut surface,
            (&projection, false),
        )
        .unwrap();
        assert_eq!(
            surface.pane_observations, expected_reports,
            "{size:?}: withheld geometry must not masquerade as all panes departing"
        );
    }
}

fn rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
    let buffer = terminal.backend().buffer();
    (buffer.area.y..buffer.area.bottom())
        .map(|row| {
            (buffer.area.x..buffer.area.right())
                .map(|column| buffer[(column, row)].symbol())
                .collect()
        })
        .collect()
}

fn text(terminal: &Terminal<TestBackend>) -> String {
    rows(terminal).join("\n")
}

fn without_cell_padding(value: &str) -> String {
    value.replace(' ', "")
}

fn expected_too_small_notice(locale: Locale, width: u16, theme: &Theme) -> String {
    let marker = format!("{} ", theme.glyphs().status_bullet());
    let marker_width =
        orchbee_workbench_client::test_support::terminal_text::display_width(&marker);
    if marker_width < usize::from(width) {
        format!(
            "{marker}{}",
            orchbee_workbench_client::test_support::terminal_text::truncate_display(
                locale.shell().too_small_notice(),
                usize::from(width) - marker_width,
                theme.glyphs().ellipsis(),
            )
        )
    } else {
        orchbee_workbench_client::test_support::terminal_text::truncate_display(
            locale.shell().too_small_notice(),
            usize::from(width),
            theme.glyphs().ellipsis(),
        )
    }
}

fn draw(
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

#[test]
fn notification_waits_for_a_presentable_root_frame_before_painting_and_settling_delivery() {
    const TITLE: &str = "Build ready 7349";
    const BODY: &str = "Agent completed 2861";
    const SMALL_WIDTH: u16 = 19;
    const SMALL_HEIGHT: u16 = 24;
    const LARGE_WIDTH: u16 = 120;
    const LARGE_HEIGHT: u16 = 40;

    let locale = Locale::EnUs;
    let theme = Theme::terminal_native();
    let mut shell = ClientView::new(SMALL_WIDTH);
    let mut policy = NotificationSettings::default();
    policy.delivery = NotificationDelivery::Tui;
    shell.notifications.configure(policy);
    assert!(
        shell.notifications.install(
            Notification::normalize(
                TITLE,
                Some(BODY),
                Some(NotificationPosition::TopRight),
                NotificationSound::None,
            )
            .unwrap(),
            Instant::now(),
        )
    );
    let pending_draw_effect = shell.notifications.clone();

    let mut terminal = Terminal::new(TestBackend::new(SMALL_WIDTH, SMALL_HEIGHT)).unwrap();
    let mut surface = Surface::default();
    let mut dispatcher = InputDispatcher::default();
    assert!(crate::notifications::available(
        &shell,
        Some(WorkbenchClientView::notification_test_view(),),
    ));
    draw(
        &mut terminal,
        &mut shell,
        &mut surface,
        &mut dispatcher,
        locale,
        &theme,
    );

    let tiny_rows = rows(&terminal);
    let visible_body = tiny_rows[..usize::from(SMALL_HEIGHT - 1)]
        .iter()
        .map(|row| without_cell_padding(row))
        .filter(|row| !row.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(
        visible_body,
        [without_cell_padding(&expected_too_small_notice(
            locale,
            SMALL_WIDTH,
            &theme,
        ))],
        "the pending card must not overwrite the root TooSmall notice"
    );
    let tiny_text = text(&terminal);
    assert!(!tiny_text.contains(TITLE));
    assert!(!tiny_text.contains(BODY));
    assert_eq!(
        shell.notifications, pending_draw_effect,
        "a frame that cannot present the card must retain its pending post-draw effect"
    );
    assert_eq!(surface.receipts, [DrawReceipt::default()]);
    assert_eq!(surface.successful_frames, 1);

    terminal.backend_mut().resize(LARGE_WIDTH, LARGE_HEIGHT);
    terminal.autoresize().unwrap();
    assert_eq!(
        terminal.get_frame().area(),
        Rect::new(0, 0, LARGE_WIDTH, LARGE_HEIGHT),
        "the test host and Terminal must agree on the restored viewport"
    );
    shell.resize(LARGE_WIDTH);
    draw(
        &mut terminal,
        &mut shell,
        &mut surface,
        &mut dispatcher,
        locale,
        &theme,
    );

    let large_text = text(&terminal);
    assert_eq!(large_text.matches(TITLE).count(), 1);
    assert_eq!(large_text.matches(BODY).count(), 1);
    assert_ne!(
        shell.notifications, pending_draw_effect,
        "the first presentable frame must settle the pending post-draw effect"
    );
    assert_eq!(
        shell.notifications.take_sound_after_draw(),
        NotificationSound::None,
        "the successful root draw already settled the one pending delivery"
    );
    let settled = shell.notifications.clone();
    assert_eq!(surface.receipts[1], DrawReceipt::default());
    assert_eq!(surface.successful_frames, 2);

    draw(
        &mut terminal,
        &mut shell,
        &mut surface,
        &mut dispatcher,
        locale,
        &theme,
    );
    assert_eq!(shell.notifications, settled);
    assert_eq!(text(&terminal).matches(TITLE).count(), 1);
    assert_eq!(surface.receipts[2], DrawReceipt::default());
    assert_eq!(surface.successful_frames, 3);
    assert!(surface.forwarded.is_empty());
}
