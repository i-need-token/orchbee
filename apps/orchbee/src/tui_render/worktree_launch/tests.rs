use super::*;
use ratatui::{Terminal, backend::TestBackend};
use std::cell::Cell;

#[test]
fn checkout_picker_bounds_unicode_rows_and_keeps_selected_path_and_footer_visible() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::rgb_test_dark()] {
            for (width, height) in [(80, 24), (120, 40), (60, 12), (50, 18), (20, 5)] {
                for selected in [0, 98, 199] {
                    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
                    let token = WorktreeToken::for_test(8);
                    let mut hits = HitMapBuilder::new();
                    hits.start_modal();
                    let visits = Cell::new(0);
                    let path = format!("C:/repo/{}/checkout", "雪".repeat(1000));
                    let mut result = None;
                    terminal
                        .draw(|frame| {
                            result = Some(render_checkouts(
                                frame,
                                locale,
                                CheckoutPicker {
                                    token,
                                    mode: WorktreeMode::Open,
                                    source: "C:/repo",
                                    count: 200,
                                    selected,
                                },
                                Rect::new(0, 0, width, height - 1),
                                &theme,
                                &mut hits,
                                |index| {
                                    visits.set(visits.get() + 1);
                                    Some(CheckoutChoice {
                                        label: format!(
                                            "branch-{index}-{}\x1b[0m",
                                            "界".repeat(200)
                                        ),
                                        path: &path,
                                        openable: true,
                                    })
                                },
                            ));
                        })
                        .unwrap();
                    let (area, presented) = result.unwrap();
                    assert!(area.bottom() < height);
                    let map = hits.finish();
                    let mut choices = std::collections::BTreeSet::new();
                    for y in 0..height {
                        for x in 0..width {
                            if let Some(HitTarget::Worktree {
                                token: painted,
                                control: ModalControl::Choice(index),
                            }) = map.hit_target_at(x, y)
                            {
                                assert_eq!(*painted, token);
                                assert!(*index < 200);
                                choices.insert(*index);
                            }
                        }
                    }
                    if width == 20 {
                        assert!(matches!(presented, ModalPresentation::NotPresented));
                        assert!(choices.is_empty());
                    } else {
                        assert!(
                            matches!(presented, ModalPresentation::Presented(value) if value == token)
                        );
                        assert!(choices.contains(&selected));
                        let text = terminal
                            .backend()
                            .buffer()
                            .content()
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>();
                        assert!(text.contains(&format!("{}/200", selected + 1)));
                        assert!(text.contains("/checkout"));
                        assert!(!text.contains('\x1b'));
                        assert!(
                            text.replace(' ', "")
                                .contains(&locale.setup_cancel_button().replace(' ', ""))
                        );
                    }
                    assert!(visits.get() <= usize::from(height / 2));
                }
            }
        }
    }
}

#[test]
fn bare_and_missing_checkout_paths_are_visible_without_activation_hits() {
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    let mut hits = HitMapBuilder::new();
    hits.start_modal();
    let theme = Theme::terminal_native();
    let mut area = Rect::default();
    terminal
        .draw(|frame| {
            area = render_checkouts(
                frame,
                Locale::EnUs,
                CheckoutPicker {
                    token: WorktreeToken::for_test(8),
                    mode: WorktreeMode::Open,
                    source: "C:/repo",
                    count: 3,
                    selected: 0,
                },
                frame.area(),
                &theme,
                &mut hits,
                |index| {
                    (index != 1).then_some(CheckoutChoice {
                        label: "branch".into(),
                        path: "C:/repo",
                        openable: index == 2,
                    })
                },
            )
            .0;
        })
        .unwrap();
    let map = hits.finish();
    let inner = super::super::overlay_surface(&theme, true, Padding::horizontal(2)).inner(area);
    for row in 2..6 {
        assert!(matches!(
            map.hit_target_at(inner.x, inner.y + row),
            Some(HitTarget::ListScroll(
                crate::tui_pointer::ListScrollTarget::Worktree(_)
            ))
        ));
    }
    assert!(matches!(
        map.hit_target_at(inner.x, inner.y + 6),
        Some(HitTarget::Worktree {
            control: ModalControl::Choice(2),
            ..
        })
    ));
    let text = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(
        text.matches(Locale::EnUs.setup_choice_unavailable())
            .count(),
        2
    );
}
