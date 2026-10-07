use super::*;
use crate::{
    client_view::Modifiers,
    terminal_input::{MouseAction, MouseButton, MouseEvent},
    tui_pointer::{HitLayer, HitMapBuilder, HitTarget},
};
use ratatui::{layout::Rect, widgets::Paragraph};

#[test]
fn action_menu_status_button_remains_visible_and_does_not_open_the_machine_picker() {
    for width in [24, 32, 50, 80, 120] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let height = 24;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut view = ClientView::new(width);
            view.machines.available = true;
            let theme = crate::tui_theme::Theme::terminal_native();
            let chrome = super::super::Chrome {
                locale,
                theme: &theme,
            };
            let button = Rect::new(width - 6, height - 1, 6, 1);
            let mut hits = HitMapBuilder::new();
            hits.record(HitLayer::Foreground, button, HitTarget::ActionMenuButton);
            let status = Rect::new(0, height - 1, width - 7, 1);
            let indicator = view.machines.indicator_area(locale, status);
            hits.record_machine_indicator(indicator);
            terminal
                .draw(|frame| {
                    frame.render_widget(Paragraph::new(locale.menu_button()), button);
                    super::super::render_machine_surface(frame, &mut view, chrome, false, &hits);
                })
                .unwrap();
            let row: String = (button.x..button.right())
                .map(|x| terminal.backend().buffer()[(x, button.y)].symbol())
                .collect();
            assert_eq!(row.replace(' ', ""), locale.menu_button());
            let packet = |column| MouseEvent {
                action: MouseAction::Press,
                button: MouseButton::Left,
                mods: Modifiers::NONE,
                column,
                row: height - 1,
            };
            for column in button.x..button.right() {
                assert!(!view.machines.pointer(packet(column)));
                assert!(!view.machines.open);
            }
            assert!(view.machines.pointer(packet(indicator.unwrap().x)));
            assert!(view.machines.open);
        }
    }
}
