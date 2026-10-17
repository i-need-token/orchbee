//! Composed category, search, fixed-action and picker evidence for the native Settings consumer.
use super::*;

#[test]
fn full_settings_body_has_one_label_value_row_per_setting_and_no_scrolling_actions() {
    let editor = editor_with(UserPreferencesValues::default());
    let theme = Theme::terminal_native();
    let (terminal, _, _) = render_once(&editor, 120, 40, Locale::EnUs, &theme);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft")
    };
    let bounds = Rect::new(0, 0, 120, 40);
    let layout =
        SettingsLayout::resolve(bounds, bounds, View::Draft(draft), Locale::EnUs, &theme).unwrap();
    let list = layout.draft.unwrap().regions.list;
    let actual: Vec<_> = (list.y..list.bottom())
        .map(|y| {
            let line: String = (list.x..list.right() - 1)
                .map(|x| terminal.backend().buffer()[(x, y)].symbol())
                .collect();
            line.trim()
                .trim_end_matches('─')
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|line| !line.is_empty())
        .collect();
    assert_eq!(
        actual,
        [
            "Appearance",
            "Language Follow system >",
            "Application theme Light >",
            "Terminal display theme Host default >",
            "Foreground color [ #141414 ]",
            "Background color [ #fcfcfc ]",
            "Terminal",
            "Copy on select [x] On",
            "Trim trailing blank rows for CLI agents [x] On",
            "Scrollback limit (bytes) [-] 10000000 B [+]",
            "Notifications",
            "Agent status notifications Never >",
            "Completion reminders Never >",
            "Minimum command duration (seconds) [-] 5 s [+]",
            "Notification sound [ ] Off",
            "Launch profiles",
            "Launch profiles System default >",
        ]
    );
}

#[test]
fn footer_commit_and_exit_are_painted_at_all_admitted_tiers_and_stay_fixed_on_scroll() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
        ] {
            for (width, height) in [(120, 40), (80, 24), (58, 12), (36, 10)] {
                let mut editor = editor_with(UserPreferencesValues::default());
                let (_, before, draw) = present_once(&mut editor, width, height, locale, &theme);
                let save = HitTarget::SettingsRow {
                    token: draw.token(),
                    row: Row::Save,
                };
                let close = HitTarget::SettingsControl {
                    token: draw.token(),
                    control: Control::Close,
                };
                let saved_cells = cells_for(&before, &save, width, height);
                assert!(!saved_cells.is_empty());
                assert!(!cells_for(&before, &close, width, height).is_empty());
                editor.scroll(draw.token(), false, 3);
                let (_, after, draw) = present_once(&mut editor, width, height, locale, &theme);
                let save = HitTarget::SettingsRow {
                    token: draw.token(),
                    row: Row::Save,
                };
                assert_eq!(
                    cells_for(&after, &save, width, height),
                    saved_cells,
                    "{locale:?} {width}x{height}"
                );
            }
        }
    }
}

#[test]
fn search_projects_only_matching_categories_and_drops_all_hidden_setting_targets() {
    let theme = Theme::one_light();
    for (locale, query) in [(Locale::EnUs, "sound"), (Locale::ZhCn, "声音")] {
        let mut editor = editor_with(UserPreferencesValues::default());
        editor.edit(Edit::Search);
        editor.paste(query);
        let (terminal, map, draw) = present_once(&mut editor, 80, 24, locale, &theme);
        let body = screen_text(&terminal);
        assert!(body.contains(locale.settings_category_name(SettingCategory::Notifications)));
        for row in [Row::Language, Row::Theme, Row::LaunchProfiles, Row::Save] {
            assert!(
                cells_for(
                    &map,
                    &HitTarget::SettingsRow {
                        token: draw.token(),
                        row
                    },
                    80,
                    24
                )
                .is_empty()
            );
        }
        assert!(
            !cells_for(
                &map,
                &HitTarget::SettingsRow {
                    token: draw.token(),
                    row: Row::Sound
                },
                80,
                24
            )
            .is_empty()
        );
        assert!(terminal.backend().cursor_visible());
        let (compact, compact_map, compact_draw) =
            present_once(&mut editor, 36, 10, locale, &theme);
        assert!(compact.backend().cursor_visible());
        assert!(
            !cells_for(
                &compact_map,
                &HitTarget::SettingsRow {
                    token: compact_draw.token(),
                    row: Row::Sound,
                },
                36,
                10
            )
            .is_empty(),
            "minimal search keeps its matching setting painted"
        );
        assert!(matches!(editor.view(), Some(View::Draft(draft))
            if draft.searching && draft.query == query && !draft.dirty
                && draft.values == &UserPreferencesValues::default()));
        for (control, label, key) in [
            (Control::Back, locale.settings_cancel(), "Esc"),
            (Control::Accept, locale.settings_accept_button(), "Enter"),
        ] {
            let target = HitTarget::SettingsControl {
                token: compact_draw.token(),
                control,
            };
            assert_eq!(
                without_cell_padding(&target_text(&compact, &compact_map, &target, false)),
                format!("[{label}{key}]")
            );
        }
        editor.edit(Edit::Clear);
        editor.paste("no-such-setting");
        let (terminal, map, draw) = present_once(&mut editor, 80, 24, locale, &theme);
        assert!(
            without_cell_padding(&screen_text(&terminal))
                .contains(&without_cell_padding(locale.settings_no_results()))
        );
        assert!((0..24).all(|y| (0..80).all(|x| !matches!(map.hit_target_at(x, y), Some(HitTarget::SettingsRow { token, .. }) if *token == draw.token()))));
    }
}

#[test]
fn same_modal_picker_keeps_parent_value_checked_and_routes_typed_pointer_acceptance() {
    let theme = Theme::one_light();
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let mut editor = editor_with(UserPreferencesValues::default());
        select_and_present(&mut editor, Row::Theme, locale, &theme);
        editor.edit(Edit::Activate);
        present_once(&mut editor, 80, 24, locale, &theme);
        editor.edit(Edit::Next);
        let (terminal, map, draw) = present_once(&mut editor, 80, 24, locale, &theme);
        assert!(screen_text(&terminal).contains(&format!(
            "{} > {}",
            locale.settings_title(),
            locale.settings_row(Row::Theme)
        )));
        let original =
            Choice::ApplicationTheme(orchbee_workbench_protocol::ApplicationAppearance::Light);
        let candidate =
            Choice::ApplicationTheme(orchbee_workbench_protocol::ApplicationAppearance::Dark);
        let original_target = HitTarget::SettingsControl {
            token: draw.token(),
            control: Control::Choose(original),
        };
        let candidate_target = HitTarget::SettingsControl {
            token: draw.token(),
            control: Control::Choose(candidate),
        };
        assert!(target_text(&terminal, &map, &original_target, false).contains("(*)"));
        assert!(target_text(&terminal, &map, &candidate_target, false).contains("( )"));
        assert!((0..24).all(|y| {
            (0..80).all(|x| !matches!(map.hit_target_at(x, y), Some(HitTarget::SettingsRow { .. })))
        }));
        assert!(matches!(editor.view(), Some(View::Draft(draft)) if !draft.dirty));
        editor.control(draw.token(), Control::Choose(candidate));
        assert!(
            matches!(editor.view(), Some(View::Draft(draft)) if draft.dirty && draft.picker.is_none() && draft.values.application_theme == orchbee_workbench_protocol::ApplicationAppearance::Dark)
        );
    }
}

#[test]
fn compact_picker_wheel_keeps_a_visible_provisional_choice_without_editing_the_parent() {
    let theme = Theme::terminal_native();
    for (locale, action_rows, visible_choices) in [(Locale::EnUs, 2, 1), (Locale::ZhCn, 1, 2)] {
        let mut editor = editor_with(UserPreferencesValues::default());
        select_and_present(&mut editor, Row::TerminalTheme, locale, &theme);
        editor.edit(Edit::Activate);
        let (terminal, map, draw) = present_once(&mut editor, 36, 10, locale, &theme);
        let bounds = Rect::new(0, 0, 36, 10);
        let layout =
            SettingsLayout::resolve(bounds, bounds, editor.view().unwrap(), locale, &theme)
                .unwrap();
        assert_eq!(
            layout.actions.height, action_rows,
            "complete English actions stack; both Chinese actions fit together"
        );
        assert_eq!(draw.visible_rows(), Some(visible_choices));
        for (control, label, key) in [
            (Control::Back, locale.settings_cancel(), "Esc"),
            (Control::Accept, locale.settings_accept_button(), "Enter"),
        ] {
            let target = HitTarget::SettingsControl {
                token: draw.token(),
                control,
            };
            assert_eq!(
                without_cell_padding(&target_text(&terminal, &map, &target, false)),
                format!("[{label}{key}]")
            );
        }
        editor.scroll(draw.token(), false, 2).unwrap();
        let (terminal, map, draw) = present_once(&mut editor, 36, 10, locale, &theme);
        assert_eq!(draw.first_visible(), Some(2));
        let choice = Choice::TerminalTheme(TerminalDisplayAppearance::Dark);
        let target = HitTarget::SettingsControl {
            token: draw.token(),
            control: Control::Choose(choice),
        };
        assert!(!target_text(&terminal, &map, &target, false).is_empty());
        assert!(
            matches!(editor.view(), Some(View::Draft(draft)) if !draft.dirty && draft.values.terminal_display_theme == TerminalDisplayAppearance::Host)
        );
        assert_eq!(
            editor.appearance_preview().unwrap().terminal_display_theme,
            TerminalDisplayAppearance::Dark
        );
        editor.edit(Edit::Cancel);
        assert!(
            matches!(editor.view(), Some(View::Draft(draft)) if draft.picker.is_none() && draft.selected == Row::TerminalTheme && !draft.dirty)
        );
        assert_eq!(
            editor.appearance_preview().unwrap().terminal_display_theme,
            TerminalDisplayAppearance::Host
        );
    }
}

#[test]
fn minimal_text_editor_keeps_exact_entry_and_complete_footer_controls() {
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::one_light()] {
            for width in [36, 40] {
                let mut editor = editor_with(UserPreferencesValues::default());
                select_and_present(&mut editor, Row::CompletionThreshold, locale, &theme);
                editor.edit(Edit::Activate);
                editor.edit(Edit::Clear);
                editor.paste("17");
                let (terminal, map, draw) = present_once(&mut editor, width, 10, locale, &theme);
                assert!(terminal.backend().cursor_visible());
                assert!(matches!(editor.view(), Some(View::Draft(draft))
                    if draft.text == Some(("17", 2))
                        && draft.values.completion_reminder_minimum_seconds == 5 && !draft.dirty));
                for (control, label, key) in [
                    (Control::Back, locale.settings_cancel(), "Esc"),
                    (Control::Accept, locale.settings_accept_button(), "Enter"),
                ] {
                    let target = HitTarget::SettingsControl {
                        token: draw.token(),
                        control,
                    };
                    assert_eq!(
                        without_cell_padding(&target_text(&terminal, &map, &target, false)),
                        format!("[{label}{key}]")
                    );
                }
                assert!((0..10).all(|y| (0..width).all(|x| !matches!(
                    map.hit_target_at(x, y),
                    Some(HitTarget::SettingsRow { .. })
                ))));
                assert!(
                    editor.control(draw.token(), Control::Accept).is_none(),
                    "child acceptance never submits Save"
                );
                assert!(matches!(editor.view(), Some(View::Draft(draft))
                    if draft.text.is_none() && draft.dirty && draft.values.completion_reminder_minimum_seconds == 17));
            }
        }
    }
}
