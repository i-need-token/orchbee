use super::*;
use ratatui::{Terminal, backend::TestBackend};
fn message(position: Position) -> Notification {
    Notification::normalize(
        "通知 ready \u{202e}abc",
        Some("中😀 e\u{301} body"),
        position,
        NotificationSound::Request,
    )
    .unwrap()
}
#[test]
fn expiry_requests_idle_redraw_and_sound_is_once_after_successful_paint_only() {
    let mut view = ClientView::new(120);
    view.settle_pointer_released(true);
    view.notifications.configure(cards());
    let original = view.clone();
    let now = Instant::now();
    assert!(view.notifications.install(message(Position::TopRight), now));
    assert!(!view.notifications.install(message(Position::TopLeft), now));
    assert!(!view.notifications.tick(now + Duration::from_secs(4)));
    assert_eq!(
        view.notifications.take_sound_after_draw(),
        NotificationSound::Request
    );
    assert_eq!(
        view.notifications.take_sound_after_draw(),
        NotificationSound::None
    );
    assert!(view.notifications.tick(now + Duration::from_secs(5)));
    assert!(!view.notifications.tick(now + Duration::from_secs(6)));
    assert_eq!(view, original, "no input/pointer/prefix/layout changes");
}

#[test]
fn automatic_delivery_does_not_inherit_or_change_manual_delivery() {
    let mut state = State::default();
    state.configure(
        WindowsNotificationSettings::from_profile(
            "[ui.toast]\ndelivery='off'\nagent_delivery='system'",
        )
        .unwrap(),
    );
    let now = Instant::now();
    assert!(state.install_automatic(message(Position::BottomRight), Delivery::System, now,));
    assert_eq!(state.take_effect().unwrap().delivery, Delivery::System);
    assert!(state.install(message(Position::BottomRight), now));
    assert_eq!(state.take_effect().unwrap().delivery, Delivery::Off);
}
#[test]
fn all_corner_cards_keep_status_and_top_chrome_unicode_safe_at_required_sizes() {
    for (width, height) in [
        (160, 50),
        (100, 30),
        (80, 24),
        (40, 12),
        (20, 6),
        (8, 5),
        (7, 3),
    ] {
        for position in [
            Position::TopLeft,
            Position::TopRight,
            Position::BottomLeft,
            Position::BottomRight,
        ] {
            let mut state = State::default();
            state.configure(cards());
            state.install(message(position), Instant::now());
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut painted = false;
            terminal
                .draw(|frame| {
                    let rect = frame.area();
                    for y in 0..height {
                        for x in 0..width {
                            frame.buffer_mut()[(x, y)].set_symbol(".");
                        }
                    }
                    painted = render(frame, &state, &Theme::rgb_test_dark());
                    assert_eq!(rect, frame.area());
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            for x in 0..width {
                assert_eq!(buffer[(x, 0)].symbol(), ".");
                assert_eq!(buffer[(x, height - 1)].symbol(), ".");
            }
            assert_eq!(painted, width >= 8 && height >= 5);
            for cell in buffer.content() {
                assert!(!cell.symbol().contains('\u{202e}'));
                assert!(!cell.symbol().contains('\x1b'));
            }
            if painted {
                let left = matches!(position, Position::TopLeft | Position::BottomLeft);
                let top = matches!(position, Position::TopLeft | Position::TopRight);
                let x = if left { 0 } else { width - 1 };
                let y = if top { 1 } else { height - 2 };
                assert_ne!(buffer[(x, y)].symbol(), ".", "corner placement");
            }
        }
    }
}

fn cards() -> WindowsNotificationSettings {
    WindowsNotificationSettings::from_profile(
        "[ui.toast]\ndelivery='orchbee'\n[ui.toast.herdr]\nposition='top-left'",
    )
    .unwrap()
}
#[test]
fn notification_policy_inherits_position_explicit_wins_and_off_keeps_custom_sound() {
    let mut state = State::default();
    let now = Instant::now();
    let custom = Notification::normalize("Ready", None, None, NotificationSound::Done).unwrap();
    state.install(custom.clone(), now);
    assert!(state.visible.is_none());
    let effect = state.take_effect().unwrap();
    assert_eq!(effect.delivery, Delivery::Off);
    assert_eq!(effect.message.sound, NotificationSound::Done);
    state.configure(cards());
    state.install(custom.clone(), now);
    assert_eq!(
        state.visible.as_ref().unwrap().message.position,
        Some(Position::TopLeft)
    );
    state.configure(WindowsNotificationSettings::default());
    assert!(state.empty());
    state.configure(cards());
    state.install(message(Position::BottomRight), now);
    assert_eq!(
        state.visible.as_ref().unwrap().message.position,
        Some(Position::BottomRight)
    );
    for delivery in ["terminal", "system"] {
        state.configure(
            WindowsNotificationSettings::from_profile(&format!(
                "[ui.toast]\ndelivery='{delivery}'"
            ))
            .unwrap(),
        );
        assert!(state.install(custom.clone(), now));
        assert!(!state.install(custom.clone(), now));
        assert!(state.visible.is_none());
        assert_eq!(state.take_effect().unwrap().delivery.as_str(), delivery);
        assert!(state.take_effect().is_none());
    }
}
