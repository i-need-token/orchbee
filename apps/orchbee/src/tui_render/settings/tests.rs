//! Settings adopts shared controls without reconstructing state or hit geometry from copy.

use super::*;
use crate::{
    client_view::KeyEvent,
    native_settings::{
        Basis, Edit, Editor, Page, Reconciliation, SETTINGS, SettingCategory, SettingsHintLabel,
        key_edit,
    },
    tui_pointer::{FrameHitMap, HelpControl},
    tui_theme::{ColorLevel, GlyphSet},
};
use orchbee_workbench_protocol::{
    PreferencesLocale, TerminalDisplayAppearance, UserPreferencesValues,
};
use ratatui::{Terminal, backend::TestBackend};
mod browse;
mod pointer;

const MISSING_SOURCE: &str = "missing";
const OTHER_SOURCE: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn basis(source: &str, values: UserPreferencesValues) -> Basis {
    Basis {
        source: source.into(),
        values,
        claimed: Default::default(),
    }
}

fn editor_with(values: UserPreferencesValues) -> Editor {
    let mut editor = Editor::default();
    let request = editor.open().expect("settings request");
    assert!(editor.loaded(request, basis(MISSING_SOURCE, values)));
    let token = editor.view().expect("loaded draft").token();
    // Compatibility admission for state-owner unit fixtures. Concrete renderer evidence below uses
    // `Editor::presented` and the exact `SettingsDraw` returned by the frame.
    editor.draw(token);
    editor
}

fn render_once(
    editor: &Editor,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (
    Terminal<TestBackend>,
    FrameHitMap,
    ModalPresentation<SettingsDraw>,
) {
    render_once_with_pending(editor, width, height, false, locale, theme)
}

fn render_once_with_pending(
    editor: &Editor,
    width: u16,
    height: u16,
    pending: bool,
    locale: Locale,
    theme: &Theme,
) -> (
    Terminal<TestBackend>,
    FrameHitMap,
    ModalPresentation<SettingsDraw>,
) {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    let mut hits = HitMapBuilder::new();
    let mut receipt = ModalPresentation::NotPresented;
    terminal
        .draw(|frame| {
            receipt = render(
                frame,
                frame.area(),
                editor.view().expect("visible settings"),
                pending,
                locale,
                theme,
                &mut hits,
            );
        })
        .unwrap();
    (terminal, hits.finish(), receipt)
}

fn present_once(
    editor: &mut Editor,
    width: u16,
    height: u16,
    locale: Locale,
    theme: &Theme,
) -> (Terminal<TestBackend>, FrameHitMap, SettingsDraw) {
    let (terminal, hits, receipt) = render_once(editor, width, height, locale, theme);
    let ModalPresentation::Presented(draw) = receipt else {
        panic!("admitted settings geometry");
    };
    editor.presented(draw);
    (terminal, hits, draw)
}

fn cells_for(map: &FrameHitMap, target: &HitTarget, width: u16, height: u16) -> Vec<(u16, u16)> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| map.hit_target_at(x, y) == Some(target))
        .collect()
}

fn target_text(
    terminal: &Terminal<TestBackend>,
    map: &FrameHitMap,
    target: &HitTarget,
    last_row_only: bool,
) -> String {
    let area = terminal.backend().buffer().area;
    let mut cells = cells_for(map, target, area.width, area.height);
    if last_row_only {
        let y = cells.iter().map(|(_, y)| *y).max().expect("painted target");
        cells.retain(|(_, row)| *row == y);
    }
    cells.sort_unstable_by_key(|&(x, y)| (y, x));
    cells
        .into_iter()
        .map(|(x, y)| terminal.backend().buffer()[(x, y)].symbol())
        .collect()
}

fn line_text(terminal: &Terminal<TestBackend>, y: u16) -> String {
    region_text(
        terminal,
        Rect::new(0, y, terminal.backend().buffer().area.width, 1),
    )
}

fn screen_text(terminal: &Terminal<TestBackend>) -> String {
    region_text(terminal, terminal.backend().buffer().area)
}

fn region_text(terminal: &Terminal<TestBackend>, area: Rect) -> String {
    let buffer = terminal.backend().buffer();
    let mut text = String::new();
    for y in area.y..area.bottom() {
        if y > area.y {
            text.push('\n');
        }
        let mut x = area.x;
        while x < area.right() {
            let symbol = buffer[(x, y)].symbol();
            text.push_str(symbol);
            // A wide glyph owns its following cells; their placeholder blanks are not text.
            x += u16::try_from(display_width(symbol).max(1)).unwrap();
        }
    }
    text
}

fn without_cell_padding(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

#[test]
fn search_and_section_landmarks_stay_secondary_without_index_or_preview_clutter() {
    use ratatui::style::Modifier;

    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [
            Theme::terminal_native(),
            Theme::one_light(),
            Theme::terminal_native().quantize(ColorLevel::Monochrome),
        ] {
            for selected in [Row::Language, Row::Theme, Row::Save] {
                let mut editor = editor_with(UserPreferencesValues::default());
                editor.edit(Edit::Select(selected));
                let view = editor.view().unwrap();
                let View::Draft(_) = view else {
                    panic!("draft")
                };
                let bounds = Rect::new(0, 0, 80, 24);
                let layout = SettingsLayout::resolve(bounds, bounds, view, locale, &theme).unwrap();
                let (terminal, _, receipt) = render_once(&editor, 80, 24, locale, &theme);
                assert!(matches!(receipt, ModalPresentation::Presented(_)));
                let buffer = terminal.backend().buffer();
                let regions = layout.draft.unwrap().regions;
                let search = &buffer[(layout.inner.x, layout.inner.y)];
                let detail = &buffer[(layout.inner.x, regions.detail.unwrap().y)];
                assert!(!search.modifier.contains(Modifier::BOLD));
                assert_eq!(search.fg, theme.style(Role::TextSecondary).fg.unwrap());
                assert!(!detail.modifier.contains(Modifier::BOLD));
                assert_eq!(detail.fg, theme.style(Role::TextSecondary).fg.unwrap());
                let text = screen_text(&terminal);
                assert!(
                    !text.contains("/16")
                        && !text.contains("Local preview;")
                        && !text.contains("本地预览；保存后")
                );
                assert!(text.contains(locale.settings_category_name(SettingCategory::Appearance)));
            }
        }
    }
}

fn footer_position(view: View<'_>, label: SettingsHintLabel) -> usize {
    footer_bindings(view)
        .iter()
        .position(|hint| hint.label() == label)
        .expect("mode exposes required semantic hint")
}

fn select_and_present(
    editor: &mut Editor,
    row: Row,
    locale: Locale,
    theme: &Theme,
) -> (Terminal<TestBackend>, FrameHitMap, SettingsDraw) {
    editor.edit(Edit::Select(row));
    present_once(editor, 120, 40, locale, theme)
}

fn open_profile_name_editor(editor: &mut Editor, locale: Locale, theme: &Theme) {
    select_and_present(editor, Row::LaunchProfiles, locale, theme);
    editor.edit(Edit::Activate);
    select_and_present(editor, Row::ProfileName, locale, theme);
    editor.edit(Edit::Activate);
    present_once(editor, 120, 40, locale, theme);
}

#[test]
fn stepper_and_footer_actions_paint_their_complete_fixed_targets() {
    for (width, height) in [(36, 10), (58, 12), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::one_dark(),
                Theme::terminal_native().quantize(ColorLevel::Monochrome),
            ] {
                let mut editor = editor_with(UserPreferencesValues::default());
                editor.edit(Edit::Select(Row::CompletionThreshold));
                let token = editor.view().unwrap().token();
                let (terminal, map, receipt) = render_once(&editor, width, height, locale, &theme);
                let ModalPresentation::Presented(draw) = receipt else {
                    panic!("{width}x{height} is admitted");
                };
                assert_eq!(draw.token(), token);
                assert!(draw.visible_rows().is_some_and(|visible| visible > 0));

                for (forward, expected) in [(false, "[-]"), (true, "[+]")] {
                    let target = HitTarget::SettingsControl {
                        token,
                        control: Control::Adjust {
                            row: Row::CompletionThreshold,
                            forward,
                        },
                    };
                    assert_eq!(
                        target_text(&terminal, &map, &target, false),
                        expected,
                        "{width}x{height} {locale:?}"
                    );
                }

                let close = HitTarget::SettingsControl {
                    token,
                    control: Control::Close,
                };
                let close_rows: std::collections::BTreeSet<_> =
                    cells_for(&map, &close, width, height)
                        .into_iter()
                        .map(|(_, row)| row)
                        .collect();
                assert_eq!(close_rows.len(), 2, "top and footer close targets");
                if theme.color(Role::SurfaceOverlay) != theme.color(Role::SurfaceChrome) {
                    let top_close_row = *close_rows.iter().min().expect("top close row");
                    let top_close_cells = cells_for(&map, &close, width, height)
                        .into_iter()
                        .filter(|(_, row)| *row == top_close_row)
                        .collect::<Vec<_>>();
                    assert_eq!(top_close_cells.len(), 3, "fixed [x] close extent");
                    assert!(top_close_cells.iter().all(|&(x, y)| {
                        terminal.backend().buffer()[(x, y)].bg == theme.color(Role::SurfaceOverlay)
                    }));
                }
                let close_text = target_text(&terminal, &map, &close, true);
                assert_eq!(
                    without_cell_padding(&close_text),
                    match locale {
                        Locale::EnUs => "[CloseEsc]",
                        Locale::ZhCn => "[关闭Esc]",
                    },
                    "the button owns both the localized verb and its binding"
                );
                let footer_row = *close_rows.iter().max().expect("footer close row");
                let hint = without_cell_padding(&line_text(&terminal, footer_row - 1));
                assert!(!hint.contains("Esc"));

                let backward = HitTarget::SettingsControl {
                    token,
                    control: Control::Adjust {
                        row: Row::CompletionThreshold,
                        forward: false,
                    },
                };
                let y = cells_for(&map, &backward, width, height)[0].1;
                assert!(line_text(&terminal, y).contains(match locale {
                    Locale::EnUs => " s",
                    Locale::ZhCn => "秒",
                }));
            }
        }
    }
}

#[test]
fn fixed_actions_use_complete_button_targets_and_keep_save_primary() {
    let theme = Theme::one_dark();
    for width in [36, 40, 120] {
        for (locale, actions) in [
            (
                Locale::EnUs,
                [
                    (Row::ResetAll, "[Resetall]"),
                    (Row::Reload, "[Reload]"),
                    (Row::Save, "[Save]"),
                ],
            ),
            (
                Locale::ZhCn,
                [
                    (Row::ResetAll, "[全部重置]"),
                    (Row::Reload, "[重新读取]"),
                    (Row::Save, "[保存]"),
                ],
            ),
        ] {
            let mut editor = editor_with(UserPreferencesValues::default());
            for (row, expected) in actions {
                editor.edit(Edit::Select(row));
                let (terminal, map, receipt) = render_once(&editor, width, 10, locale, &theme);
                let ModalPresentation::Presented(draw) = receipt else {
                    panic!("{width}x10 settings geometry");
                };
                editor.presented(draw);
                let target = HitTarget::SettingsRow {
                    token: draw.token(),
                    row,
                };
                let cells = cells_for(&map, &target, width, 10);
                assert!(!cells.is_empty(), "{row:?} at {width}x10 {locale:?}");
                let y = cells[0].1;
                assert!(cells.iter().all(|(_, cell_y)| *cell_y == y));
                let minimum = cells.iter().map(|(x, _)| *x).min().unwrap();
                let maximum = cells.iter().map(|(x, _)| *x).max().unwrap();
                assert_eq!(cells.len(), usize::from(maximum - minimum + 1));
                assert!(cells.len() >= display_width(expected));
                let expected = if row == Row::Save && width >= 44 {
                    match locale {
                        Locale::EnUs => "[SaveCtrl+s]",
                        Locale::ZhCn => "[保存Ctrl+s]",
                    }
                } else {
                    expected
                };
                assert!(
                    without_cell_padding(&line_text(&terminal, y)).contains(expected),
                    "{row:?} at {width}x10 {locale:?}"
                );
            }
        }
    }

    let mut clean = editor_with(UserPreferencesValues::default());
    let (terminal, map, draw) = select_and_present(&mut clean, Row::Save, Locale::EnUs, &theme);
    let target = HitTarget::SettingsRow {
        token: draw.token(),
        row: Row::Save,
    };
    let save = cells_for(&map, &target, 120, 40)
        .into_iter()
        .find(|&(x, y)| terminal.backend().buffer()[(x, y)].symbol() == "S")
        .expect("disabled Save label");
    let disabled = theme.content_on_surface(
        Role::SurfaceOverlay,
        None,
        Some(Role::TextDisabled),
        theme.style(Role::TextDisabled),
    );
    assert_eq!(terminal.backend().buffer()[save].style(), disabled);

    let mut dirty = editor_with(UserPreferencesValues::default());
    select_and_present(&mut dirty, Row::Sound, Locale::EnUs, &theme);
    dirty.edit(Edit::Adjust(true));
    let (terminal, map, draw) = select_and_present(&mut dirty, Row::Save, Locale::EnUs, &theme);
    let target = HitTarget::SettingsRow {
        token: draw.token(),
        row: Row::Save,
    };
    let save = cells_for(&map, &target, 120, 40)
        .into_iter()
        .find(|&(x, y)| terminal.backend().buffer()[(x, y)].symbol() == "S")
        .expect("enabled Save label");
    let enabled = theme.content_on_surface(
        Role::SurfaceOverlay,
        None,
        Some(Role::AccentAction),
        theme.style(Role::AccentAction),
    );
    assert_eq!(terminal.backend().buffer()[save].style(), enabled);
}

#[test]
fn footer_hints_split_key_and_label_roles_and_drop_only_whole_suffix_items() {
    let theme = Theme::one_dark();
    for (width, locale, expected) in [
        (36, Locale::EnUs, "Enter activate/edit  │  / search"),
        (40, Locale::EnUs, "Enter activate/edit  │  / search"),
        (36, Locale::ZhCn, "Enter 操作／编辑  │  / 搜索"),
    ] {
        let editor = editor_with(UserPreferencesValues::default());
        let (terminal, _, receipt) = render_once(&editor, width, 10, locale, &theme);
        assert!(matches!(receipt, ModalPresentation::Presented(_)));
        let (y, line) = (0..10)
            .map(|y| (y, line_text(&terminal, y)))
            .find(|(_, line)| line.contains("Enter"))
            .expect("footer primary hint");
        let expected = without_cell_padding(expected);
        assert!(
            without_cell_padding(&line).contains(expected.as_str()),
            "{width} {locale:?}: {line:?}"
        );
        assert!(line.contains('/'));
        assert!(!line.contains("Esc"), "exit belongs to its button");
        assert!(!line.contains('←'), "whole fourth item must be dropped");

        let key_x = (0..width)
            .find(|&x| terminal.backend().buffer()[(x, y)].symbol() == "E")
            .expect("Enter key span");
        let label_x = key_x + 6;
        let key_style = terminal.backend().buffer()[(key_x, y)].style();
        let label_style = terminal.backend().buffer()[(label_x, y)].style();
        assert_eq!(key_style.fg, theme.style(Role::HintKey).fg);
        assert_eq!(label_style.fg, theme.style(Role::HintLabel).fg);
        assert!(
            key_style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
        assert!(
            !label_style
                .add_modifier
                .contains(ratatui::style::Modifier::BOLD)
        );
    }

    let ascii_theme = Theme::one_dark().with_glyphs(GlyphSet::ASCII);
    let editor = editor_with(UserPreferencesValues::default());
    let (terminal, _, _) = render_once(&editor, 120, 10, Locale::EnUs, &ascii_theme);
    let footer = (0..10)
        .map(|y| line_text(&terminal, y))
        .find(|line| line.contains("Enter activate/edit"))
        .expect("simplified footer");
    assert!(footer.contains("/ search"));
    assert!(!footer.contains('←') && !footer.contains('→'));
}

#[test]
fn confirmation_bindings_are_inside_complete_action_targets_without_a_duplicate_hint_row() {
    for (width, height) in [(80, 10), (80, 24), (120, 40)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for theme in [
                Theme::one_dark(),
                Theme::one_light(),
                Theme::terminal_native(),
            ] {
                let mut editor = editor_with(UserPreferencesValues::default());
                select_and_present(&mut editor, Row::ResetAll, locale, &theme);
                editor.edit(Edit::Activate);
                let (terminal, map, receipt) = render_once(&editor, width, height, locale, &theme);
                let ModalPresentation::Presented(draw) = receipt else {
                    panic!("complete confirmation actions at {width}x{height} {locale:?}");
                };
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
                        format!("[{label}{key}]"),
                    );
                    let cells = cells_for(&map, &target, width, height);
                    let y = cells[0].1;
                    assert!(cells.iter().all(|(_, row)| *row == y));
                    let hint_row = line_text(&terminal, y - 1);
                    assert!(!hint_row.contains("Esc") && !hint_row.contains("Enter"));
                    let key_x = cells
                        .iter()
                        .find_map(|&(x, row)| {
                            (terminal.backend().buffer()[(x, row)].symbol() == "E").then_some(x)
                        })
                        .expect("shortcut is inside the clickable action");
                    let style = terminal.backend().buffer()[(key_x, y)].style();
                    assert_eq!(style.fg, theme.style(Role::TextSecondary).fg);
                    assert!(!style.add_modifier.contains(ratatui::style::Modifier::BOLD));
                }
            }
        }
    }

    let theme = Theme::one_dark();
    let mut editor = editor_with(UserPreferencesValues::default());
    select_and_present(&mut editor, Row::ResetAll, Locale::EnUs, &theme);
    editor.edit(Edit::Activate);
    let (_, map, receipt) = render_once(&editor, 35, 10, Locale::EnUs, &theme);
    assert!(matches!(receipt, ModalPresentation::NotPresented));
    assert!((0..10).all(|y| (0..35).all(|x| map.hit_target_at(x, y).is_none())));
}

#[test]
fn confirmation_admission_keeps_the_complete_localized_decision_and_both_actions() {
    let theme = Theme::one_dark();
    for (width, locale, admitted) in [
        (35, Locale::EnUs, false),
        (35, Locale::ZhCn, false),
        (36, Locale::EnUs, true),
        (36, Locale::ZhCn, true),
        (48, Locale::EnUs, true),
        (48, Locale::ZhCn, true),
        (60, Locale::EnUs, true),
    ] {
        let mut editor = editor_with(UserPreferencesValues::default());
        select_and_present(&mut editor, Row::ResetAll, locale, &theme);
        editor.edit(Edit::Activate);
        let (terminal, map, receipt) = render_once(&editor, width, 10, locale, &theme);
        assert_eq!(matches!(receipt, ModalPresentation::Presented(_)), admitted);
        if admitted {
            let ModalPresentation::Presented(draw) = receipt else {
                unreachable!();
            };
            let View::Draft(draft) = editor.view().unwrap() else {
                panic!("confirmation draft")
            };
            let bounds = Rect::new(0, 0, width, 10);
            let layout =
                SettingsLayout::resolve(bounds, bounds, View::Draft(draft), locale, &theme)
                    .unwrap();
            let description = Rect::new(layout.inner.x, layout.inner.y + 1, layout.inner.width, 2);
            // Read the decision slot: screen borders separate its wrapped lines, and wide-glyph
            // continuation cells are not additional spaces in the localized decision.
            assert_eq!(
                without_cell_padding(&region_text(&terminal, description)),
                without_cell_padding(&locale.settings_detail(draft))
            );
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
                assert!(
                    cells_for(&map, &target, width, 10)
                        .iter()
                        .all(|&(_, y)| y >= description.bottom())
                );
            }
        } else {
            assert!((0..10).all(|y| (0..width).all(|x| map.hit_target_at(x, y).is_none())));
        }
    }
}

#[test]
fn footer_projection_uses_live_local_bindings_and_mode_semantics() {
    let theme = Theme::one_dark();

    let assert_live = |view: View<'_>, text_entry: bool| {
        for hint in footer_bindings(view) {
            for (index, binding) in hint.bindings().enumerate() {
                let edit = key_edit(KeyEvent::press(binding.code, binding.mods), text_entry);
                let matches_intent = match hint.label() {
                    SettingsHintLabel::Close
                    | SettingsHintLabel::Back
                    | SettingsHintLabel::Cancel => matches!(edit, Some(Edit::Cancel)),
                    SettingsHintLabel::Confirm
                    | SettingsHintLabel::Accept
                    | SettingsHintLabel::ActivateEdit => matches!(edit, Some(Edit::Activate)),
                    SettingsHintLabel::Select => matches!(edit, Some(Edit::Next)),
                    SettingsHintLabel::Change => matches!(
                        (index, edit),
                        (0, Some(Edit::Adjust(false))) | (1, Some(Edit::Adjust(true)))
                    ),
                    SettingsHintLabel::Clear => matches!(edit, Some(Edit::Clear)),
                    SettingsHintLabel::Search => matches!(edit, Some(Edit::Search)),
                    SettingsHintLabel::Save => matches!(edit, Some(Edit::Save)),
                };
                assert!(
                    matches_intent,
                    "every displayed binding must dispatch its advertised local intent"
                );
            }
        }
    };

    let mut loading = Editor::default();
    loading.open().expect("settings request");
    let loading_view = loading.view().expect("loading view");
    assert_eq!(footer_bindings(loading_view).len(), 1);
    assert_eq!(
        footer_bindings(loading_view)[0].label(),
        SettingsHintLabel::Close
    );
    assert_live(loading_view, false);

    let general = editor_with(UserPreferencesValues::default());
    let general_view = general.view().expect("general draft");
    assert_eq!(footer_position(general_view, SettingsHintLabel::Close), 0);
    assert_eq!(
        footer_position(general_view, SettingsHintLabel::ActivateEdit),
        2
    );
    assert_eq!(footer_bindings(general_view).len(), 4);
    assert_eq!(
        footer_bindings(general_view)[footer_position(general_view, SettingsHintLabel::Search)]
            .bindings()
            .len(),
        1
    );
    assert_live(general_view, false);

    let mut profiles = editor_with(UserPreferencesValues::default());
    select_and_present(&mut profiles, Row::LaunchProfiles, Locale::EnUs, &theme);
    profiles.edit(Edit::Activate);
    let profiles_view = profiles.view().expect("profiles draft");
    assert_eq!(footer_position(profiles_view, SettingsHintLabel::Back), 0);
    assert_eq!(
        footer_position(profiles_view, SettingsHintLabel::ActivateEdit),
        1
    );
    assert_live(profiles_view, false);

    let mut confirmation = editor_with(UserPreferencesValues::default());
    select_and_present(&mut confirmation, Row::ResetAll, Locale::EnUs, &theme);
    confirmation.edit(Edit::Activate);
    let confirmation_view = confirmation.view().expect("confirmation draft");
    assert_eq!(
        footer_position(confirmation_view, SettingsHintLabel::Cancel),
        0
    );
    assert_eq!(
        footer_position(confirmation_view, SettingsHintLabel::Confirm),
        1
    );
    assert_live(confirmation_view, false);

    let mut text = editor_with(UserPreferencesValues::default());
    open_profile_name_editor(&mut text, Locale::EnUs, &theme);
    let text_view = text.view().expect("text draft");
    assert_eq!(footer_position(text_view, SettingsHintLabel::Cancel), 0);
    assert_eq!(footer_position(text_view, SettingsHintLabel::Accept), 1);
    assert!(
        footer_position(text_view, SettingsHintLabel::Clear)
            > footer_position(text_view, SettingsHintLabel::Accept)
    );
    assert_live(text_view, true);
    let (terminal, _, _) = render_once(&text, 120, 40, Locale::EnUs, &theme);
    let clear = &footer_bindings(text_view)[footer_position(text_view, SettingsHintLabel::Clear)];
    let clear_key = clear
        .bindings()
        .map(|binding| {
            format_binding_with_glyphs(&binding, LabelStyle::for_host(), KeyLabelGlyphs::Unicode)
        })
        .collect::<Vec<_>>()
        .join("/");
    assert!(screen_text(&terminal).contains(&format!("{clear_key} clear")));

    let mut saving = editor_with(UserPreferencesValues::default());
    select_and_present(&mut saving, Row::Sound, Locale::EnUs, &theme);
    saving.edit(Edit::Adjust(true));
    select_and_present(&mut saving, Row::Save, Locale::EnUs, &theme);
    saving.edit(Edit::Activate).expect("dirty save");
    let saving_view = saving.view().expect("saving view");
    assert_eq!(footer_bindings(saving_view).len(), 1);
    assert_eq!(
        footer_bindings(saving_view)[0].label(),
        SettingsHintLabel::Close
    );
    assert_live(saving_view, false);
    let (terminal, _, _) = render_once(&saving, 40, 10, Locale::EnUs, &theme);
    assert!(screen_text(&terminal).contains("Saving continues after close"));
    let (terminal, _, receipt) =
        render_once_with_pending(&saving, 40, 10, true, Locale::EnUs, &theme);
    assert!(matches!(receipt, ModalPresentation::Presented(_)));
    let text = screen_text(&terminal);
    assert!(text.contains("Saving continues after close"));
    assert!(text.contains("[..] Saving"));
}

#[test]
fn unavailable_stepper_endpoint_stays_visible_and_consumes_its_cells() {
    let values = UserPreferencesValues {
        completion_reminder_minimum_seconds: 1,
        ..UserPreferencesValues::default()
    };
    let mut editor = editor_with(values);
    editor.edit(Edit::Select(Row::CompletionThreshold));
    let token = editor.view().unwrap().token();
    let theme = Theme::one_dark();
    let (terminal, map, receipt) = render_once(&editor, 120, 40, Locale::EnUs, &theme);
    assert!(matches!(receipt, ModalPresentation::Presented(_)));

    let backward = HitTarget::SettingsControl {
        token,
        control: Control::Adjust {
            row: Row::CompletionThreshold,
            forward: false,
        },
    };
    let forward = HitTarget::SettingsControl {
        token,
        control: Control::Adjust {
            row: Row::CompletionThreshold,
            forward: true,
        },
    };
    assert_eq!(target_text(&terminal, &map, &backward, false), "[-]");
    assert_eq!(target_text(&terminal, &map, &forward, false), "[+]");
    let back_cells = cells_for(&map, &backward, 120, 40);
    let forward_cells = cells_for(&map, &forward, 120, 40);
    assert_eq!(back_cells.len(), 3);
    assert_eq!(forward_cells.len(), 3);
    assert_ne!(
        terminal.backend().buffer()[back_cells[1]].style().fg,
        terminal.backend().buffer()[forward_cells[1]].style().fg,
        "the unavailable endpoint keeps disabled treatment without becoming a dead strip"
    );
}

#[test]
fn current_setting_uses_a_neutral_band_and_preserves_its_disabled_identity() {
    let theme = Theme::one_dark();
    let values = UserPreferencesValues {
        terminal_display_theme: TerminalDisplayAppearance::Host,
        ..UserPreferencesValues::default()
    };
    let mut editor = editor_with(values);
    let (terminal, map, draw) =
        select_and_present(&mut editor, Row::Foreground, Locale::EnUs, &theme);
    let target = HitTarget::SettingsRow {
        token: draw.token(),
        row: Row::Foreground,
    };
    let cells = cells_for(&map, &target, 120, 40);
    assert!(!cells.is_empty());
    let (row_x, row_y) = *cells.iter().min_by_key(|(x, _)| *x).unwrap();
    assert_eq!(terminal.backend().buffer()[(row_x, row_y)].symbol(), " ");
    assert_ne!(
        terminal.backend().buffer()[(row_x + 5, row_y)].symbol(),
        theme.glyphs.active_marker,
        "the enclosing frame owns focus; the row does not repeat its focus marker"
    );
    assert!(line_text(&terminal, row_y).contains("Foreground"));
    let (foreground, _) = theme.terminal_rgb_defaults().unwrap();
    assert!(line_text(&terminal, row_y).contains(&format!("#{foreground:06x}")));
    assert!(!line_text(&terminal, row_y).contains("#141414"));
    let label = (row_x..120)
        .find(|&x| terminal.backend().buffer()[(x, row_y)].symbol() == "F")
        .unwrap();
    assert!(
        terminal.backend().buffer()[(label, row_y)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD)
    );
}

#[test]
fn boolean_focus_checked_and_changed_cues_remain_independent() {
    let values = UserPreferencesValues {
        copy_on_select: false,
        ..UserPreferencesValues::default()
    };
    let mut editor = editor_with(values);
    let theme = Theme::terminal_native().quantize(ColorLevel::Monochrome);
    let (off_terminal, off_map, off_draw) =
        select_and_present(&mut editor, Row::CopyOnSelect, Locale::EnUs, &theme);
    let off_target = HitTarget::SettingsRow {
        token: off_draw.token(),
        row: Row::CopyOnSelect,
    };
    let off_cells = cells_for(&off_map, &off_target, 120, 40);
    let (row_x, row_y) = *off_cells.iter().min_by_key(|(x, _)| *x).unwrap();
    assert!(line_text(&off_terminal, row_y).contains("[ ] "));
    assert_eq!(
        off_terminal.backend().buffer()[(row_x, row_y)].symbol(),
        " "
    );
    assert_eq!(
        off_terminal.backend().buffer()[(row_x + 1, row_y)].symbol(),
        " "
    );
    assert_ne!(
        off_terminal.backend().buffer()[(row_x + 5, row_y)].symbol(),
        theme.glyphs.active_marker
    );

    editor.edit(Edit::Adjust(true));
    let (on_terminal, on_map, on_draw) = present_once(&mut editor, 120, 40, Locale::EnUs, &theme);
    let on_target = HitTarget::SettingsRow {
        token: on_draw.token(),
        row: Row::CopyOnSelect,
    };
    let on_cells = cells_for(&on_map, &on_target, 120, 40);
    let (row_x, row_y) = *on_cells.iter().min_by_key(|(x, _)| *x).unwrap();
    assert!(line_text(&on_terminal, row_y).contains("[x] "));
    assert_eq!(on_terminal.backend().buffer()[(row_x, row_y)].symbol(), " ");
    assert_eq!(
        on_terminal.backend().buffer()[(row_x + 1, row_y)].symbol(),
        "~"
    );
    assert_ne!(
        on_terminal.backend().buffer()[(row_x + 5, row_y)].symbol(),
        theme.glyphs.active_marker
    );
    let text = screen_text(&on_terminal);
    assert!(text.contains("Unsaved changes"));
    assert!(!text.contains("basis/effective"));
    assert!(!text.contains("source"));
}

#[test]
fn choice_and_editor_boundaries_remain_visible_at_rest_and_during_editing() {
    let theme = Theme::one_light();
    let mut editor = editor_with(UserPreferencesValues::default());
    let (choice_terminal, choice_map, draw) =
        select_and_present(&mut editor, Row::Language, Locale::EnUs, &theme);
    let target = HitTarget::SettingsRow {
        token: draw.token(),
        row: Row::Language,
    };
    let y = cells_for(&choice_map, &target, 120, 40)[0].1;
    let choice = line_text(&choice_terminal, y);
    assert!(choice.contains("Follow system >"));
    assert!(
        !choice.contains('['),
        "a finite value is one calm row, not a nested button"
    );

    open_profile_name_editor(&mut editor, Locale::EnUs, &theme);
    let (terminal, map, draw) = present_once(&mut editor, 120, 40, Locale::EnUs, &theme);
    assert!(screen_text(&terminal).contains("Name: ["));
    assert!(terminal.backend().cursor_visible());
    assert!((0..40).all(|row| (0..120).all(|column| !matches!(
        map.hit_target_at(column, row),
        Some(HitTarget::SettingsRow { .. })
    ))));
    for (control, label) in [
        (Control::Back, "[ Cancel  Esc ]"),
        (Control::Accept, "[ Confirm  Enter ]"),
    ] {
        let target = HitTarget::SettingsControl {
            token: draw.token(),
            control,
        };
        let text = target_text(&terminal, &map, &target, false);
        assert!(text.contains(label), "{text:?}");
    }
}

#[test]
fn invalid_editor_accept_remains_visible_as_a_disabled_typed_target() {
    let theme = Theme::one_light();
    let mut editor = editor_with(UserPreferencesValues::default());
    open_profile_name_editor(&mut editor, Locale::EnUs, &theme);
    editor.edit(Edit::Clear);
    editor.paste(" ");
    editor.edit(Edit::Activate);
    let token = editor.view().expect("invalid editor stays open").token();
    let (terminal, map, receipt) = render_once(&editor, 120, 40, Locale::EnUs, &theme);
    assert!(matches!(receipt, ModalPresentation::Presented(_)));
    assert!(screen_text(&terminal).contains("[ Confirm  Enter ]"));

    let cancel = HitTarget::SettingsControl {
        token,
        control: Control::Back,
    };
    let accept = HitTarget::SettingsControl {
        token,
        control: Control::Accept,
    };
    assert!(!cells_for(&map, &cancel, 120, 40).is_empty());
    let accept_cells = cells_for(&map, &accept, 120, 40);
    assert!(!accept_cells.is_empty());
    assert_eq!(
        target_text(&terminal, &map, &accept, false),
        " [ Confirm  Enter ]"
    );
    let expected = theme.content_on_surface(
        Role::SurfaceOverlay,
        None,
        Some(Role::TextDisabled),
        theme.style(Role::TextDisabled),
    );
    assert!(
        accept_cells
            .iter()
            .all(|&(x, y)| terminal.backend().buffer()[(x, y)].style() == expected)
    );
}

#[test]
fn setting_category_copy_projects_the_closed_metadata_inventory() {
    assert_eq!(SETTINGS.len(), 13);
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for metadata in SETTINGS {
            let expected = match (locale, metadata.category) {
                (Locale::EnUs, SettingCategory::Appearance) => "Appearance",
                (Locale::ZhCn, SettingCategory::Appearance) => "外观",
                (Locale::EnUs, SettingCategory::Terminal) => "Terminal",
                (Locale::ZhCn, SettingCategory::Terminal) => "终端",
                (Locale::EnUs, SettingCategory::Notifications) => "Notifications",
                (Locale::ZhCn, SettingCategory::Notifications) => "通知",
                (Locale::EnUs, SettingCategory::LaunchProfiles) => "Launch profiles",
                (Locale::ZhCn, SettingCategory::LaunchProfiles) => "启动配置",
            };
            assert_eq!(locale.settings_category(metadata.row), expected);
        }
        assert_eq!(
            locale.settings_category(Row::Save),
            match locale {
                Locale::EnUs => "General",
                Locale::ZhCn => "通用",
            }
        );
        assert_eq!(
            locale.settings_category(Row::ProfileName),
            match locale {
                Locale::EnUs => "Launch profiles",
                Locale::ZhCn => "启动配置",
            }
        );
    }
}

#[test]
fn disabled_save_detail_identifies_clean_conflict_and_reconciliation_reasons() {
    let theme = Theme::one_dark();

    let mut clean = editor_with(UserPreferencesValues::default());
    select_and_present(&mut clean, Row::Save, Locale::EnUs, &theme);
    let View::Draft(clean) = clean.view().expect("clean draft") else {
        panic!("clean draft view");
    };
    assert_eq!(Locale::EnUs.settings_detail(clean), "No changes to save.");
    assert_eq!(Locale::ZhCn.settings_detail(clean), "没有需要保存的更改。");
    assert_eq!(
        Locale::EnUs.settings_state_facts(clean),
        "No unsaved changes"
    );
    assert_eq!(Locale::ZhCn.settings_state_facts(clean), "没有未保存的更改");

    let mut conflict = editor_with(UserPreferencesValues::default());
    select_and_present(&mut conflict, Row::Sound, Locale::EnUs, &theme);
    conflict.edit(Edit::Adjust(true));
    let mut effective = UserPreferencesValues::default();
    effective.sound_enabled = !effective.sound_enabled;
    assert_eq!(
        conflict.observe(&basis(OTHER_SOURCE, effective)),
        crate::native_settings::Observation::Conflict
    );
    select_and_present(&mut conflict, Row::Save, Locale::EnUs, &theme);
    let View::Draft(conflict) = conflict.view().expect("conflicted draft") else {
        panic!("conflicted draft view");
    };
    assert_eq!(
        Locale::EnUs.settings_detail(conflict),
        "Settings changed elsewhere; reload before saving."
    );
    assert_eq!(
        Locale::ZhCn.settings_detail(conflict),
        "另一端已修改设置；请重新读取后再保存。"
    );

    for (reason, english, chinese) in [
        (
            Reconciliation::SourceChanged,
            "Source changed; reload before saving.",
            "来源已变化；请重新读取后再保存。",
        ),
        (
            Reconciliation::OutcomeUnknown,
            "Save outcome unknown; reload before saving again.",
            "保存结果未知；请重新读取后再保存。",
        ),
    ] {
        let mut editor = editor_with(UserPreferencesValues::default());
        select_and_present(&mut editor, Row::Sound, Locale::EnUs, &theme);
        editor.edit(Edit::Adjust(true));
        select_and_present(&mut editor, Row::Save, Locale::EnUs, &theme);
        let submission = editor.edit(Edit::Activate).expect("dirty save");
        assert!(editor.failed(submission.token, Some(reason)));
        let View::Draft(draft) = editor.view().expect("retained draft") else {
            panic!("retained draft view");
        };
        assert_eq!(Locale::EnUs.settings_detail(draft), english);
        assert_eq!(Locale::ZhCn.settings_detail(draft), chinese);
    }
}

#[test]
fn manual_scroll_can_hide_the_anchor_without_painting_false_focus_or_authorizing_enter() {
    let theme = Theme::one_dark();
    let mut editor = editor_with(UserPreferencesValues::default());
    let (_, _, first) = present_once(&mut editor, 36, 10, Locale::EnUs, &theme);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft");
    };
    let entries = draft.entries();
    let values = draft.values.clone();
    let anchor = entries
        .iter()
        .position(|entry| *entry == Entry::Setting(Row::Language))
        .unwrap();
    let destination = entries
        .iter()
        .position(|entry| *entry == Entry::Setting(Row::CopyOnSelect))
        .unwrap();
    assert_eq!(
        first.first_visible(),
        Some(anchor),
        "one visible row reveals the setting, not its category heading"
    );
    assert_eq!(first.visible_rows(), Some(1));
    let scrolled = editor
        .scroll(first.token(), false, destination - anchor)
        .expect("painted list admits wheel scrolling");
    let (terminal, map, receipt) = render_once(&editor, 36, 10, Locale::EnUs, &theme);
    let ModalPresentation::Presented(draw) = receipt else {
        panic!("scrolled settings stays admitted");
    };
    assert_eq!(draw.token(), scrolled);
    assert_eq!(draw.first_visible(), Some(destination));
    editor.presented(draw);

    let visible_rows: Vec<_> = entries[destination..destination + draw.visible_rows().unwrap()]
        .iter()
        .filter_map(|entry| {
            let Entry::Setting(row) = *entry else {
                return None;
            };
            let target = HitTarget::SettingsRow {
                token: draw.token(),
                row,
            };
            let cells = cells_for(&map, &target, 36, 10);
            (!cells.is_empty()).then_some(cells)
        })
        .collect();
    assert!(!visible_rows.is_empty());
    assert!(
        cells_for(
            &map,
            &HitTarget::SettingsRow {
                token: draw.token(),
                row: Row::Language
            },
            36,
            10
        )
        .is_empty()
    );
    for cells in visible_rows {
        let (x, y) = *cells.iter().min_by_key(|(x, _)| *x).unwrap();
        assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), " ");
        let geometry =
            RowGeometry::new(Rect::new(x, y, u16::try_from(cells.len()).unwrap(), 1)).unwrap();
        for x in geometry.primary.x..geometry.primary.right() {
            let cell = &terminal.backend().buffer()[(x, y)];
            assert!(!cell.modifier.contains(ratatui::style::Modifier::BOLD));
            assert_eq!(cell.bg, theme.color(Role::SurfaceOverlay));
        }
    }

    let before = editor.view().unwrap().token();
    editor.edit(Edit::Activate);
    let View::Draft(draft) = editor.view().unwrap() else {
        panic!("draft remains open");
    };
    assert_eq!(draft.token, before);
    assert_eq!(draft.values.locale, PreferencesLocale::System);
    assert_eq!(draft.values, &values);
    assert_eq!(draft.selected, Row::Language);
    assert!(
        draft.picker.is_none(),
        "the hidden anchor cannot open a selector"
    );

    editor.edit(Edit::Next);
    let (_, _, receipt) = render_once(&editor, 36, 10, Locale::EnUs, &theme);
    let ModalPresentation::Presented(revealed) = receipt else {
        panic!("keyboard navigation remains admitted");
    };
    assert_eq!(
        revealed.first_visible(),
        entries
            .iter()
            .position(|entry| *entry == Entry::Setting(Row::Theme))
    );
}

#[test]
fn footer_status_copy_prioritizes_conflict_and_reconciliation_over_draft_changes() {
    let theme = Theme::one_dark();
    let mut editor = editor_with(UserPreferencesValues::default());
    select_and_present(&mut editor, Row::Sound, Locale::EnUs, &theme);
    editor.edit(Edit::Adjust(true));
    let mut effective = UserPreferencesValues::default();
    effective.sound_enabled = !effective.sound_enabled;
    assert_eq!(
        editor.observe(&basis(OTHER_SOURCE, effective)),
        crate::native_settings::Observation::Conflict
    );
    let View::Draft(conflict) = editor.view().expect("conflicted draft") else {
        panic!("conflicted draft view");
    };
    assert_eq!(
        Locale::EnUs.settings_state_facts(conflict),
        "Saved settings changed elsewhere · Reload required"
    );
    assert_eq!(
        Locale::ZhCn.settings_state_facts(conflict),
        "已保存的设置已在其他位置更改 · 需要重新读取"
    );
    let (terminal, _, _) = present_once(&mut editor, 120, 40, Locale::EnUs, &theme);
    let text = screen_text(&terminal);
    assert!(text.contains("Saved settings changed elsewhere · Reload required"));
    assert!(!text.contains(OTHER_SOURCE));
    let (terminal, _, _) = present_once(&mut editor, 120, 40, Locale::ZhCn, &theme);
    assert!(
        without_cell_padding(&screen_text(&terminal))
            .contains("已保存的设置已在其他位置更改·需要重新读取")
    );

    for (reason, english, chinese) in [
        (
            Reconciliation::SourceChanged,
            "Saved settings changed elsewhere · Reload required",
            "已保存的设置已在其他位置更改 · 需要重新读取",
        ),
        (
            Reconciliation::OutcomeUnknown,
            "Save result unknown · Reload required",
            "保存结果未知 · 需要重新读取",
        ),
    ] {
        let mut editor = editor_with(UserPreferencesValues::default());
        select_and_present(&mut editor, Row::Sound, Locale::EnUs, &theme);
        editor.edit(Edit::Adjust(true));
        select_and_present(&mut editor, Row::Save, Locale::EnUs, &theme);
        let submission = editor.edit(Edit::Activate).expect("dirty save");
        assert!(editor.failed(submission.token, Some(reason)));
        let View::Draft(draft) = editor.view().expect("retained draft") else {
            panic!("retained draft view");
        };
        assert_eq!(Locale::EnUs.settings_state_facts(draft), english);
        assert_eq!(Locale::ZhCn.settings_state_facts(draft), chinese);
        let (terminal, _, _) = present_once(&mut editor, 120, 40, Locale::EnUs, &theme);
        assert!(screen_text(&terminal).contains(english));
        let (terminal, _, _) = present_once(&mut editor, 120, 40, Locale::ZhCn, &theme);
        let chinese = without_cell_padding(chinese);
        assert!(without_cell_padding(&screen_text(&terminal)).contains(chinese.as_str()));
    }
}

#[test]
fn text_cursor_stays_inside_the_overlay_and_editor_text_is_sanitized() {
    let theme = Theme::one_light();
    let mut editor = editor_with(UserPreferencesValues::default());
    open_profile_name_editor(&mut editor, Locale::ZhCn, &theme);
    editor.paste(&format!("{}\x1b[31m", "长名".repeat(50)));
    let (terminal, _, _) = present_once(&mut editor, 40, 12, Locale::ZhCn, &theme);
    assert!(!screen_text(&terminal).contains('\u{1b}'));
    assert!(terminal.backend().cursor_visible());
}

fn assert_projected_text_cursor(raw: &str, glyph: &str, right_column: u16, end_column: u16) {
    let theme = Theme::one_light();
    let mut editor = editor_with(UserPreferencesValues::default());
    open_profile_name_editor(&mut editor, Locale::EnUs, &theme);
    editor.edit(Edit::Clear);
    editor.paste(raw);
    for (edit, column) in [
        (Edit::Home, 0),
        (Edit::Adjust(true), right_column),
        (Edit::End, end_column),
    ] {
        editor.edit(edit);
        let (mut terminal, _, _) = present_once(&mut editor, 58, 12, Locale::EnUs, &theme);
        let buffer = terminal.backend().buffer();
        let index = buffer
            .content
            .iter()
            .position(|cell| cell.symbol() == glyph)
            .expect("the complete projected grapheme is painted");
        let stride = usize::from(buffer.area.width);
        let origin =
            ratatui::layout::Position::new((index % stride) as u16, (index / stride) as u16);
        assert_eq!(
            terminal.get_cursor_position().unwrap(),
            ratatui::layout::Position::new(origin.x + column, origin.y),
            "cursor after {edit:?} for raw {raw:?}"
        );
        assert!(terminal.backend().cursor_visible());
        assert!(
            matches!(editor.view(), Some(View::Draft(view)) if view.text.is_some_and(|(text, _)| text == raw))
        );
    }
}

#[test]
fn settings_text_cursor_snaps_before_a_combining_cluster_joined_by_stripping_controls() {
    assert_projected_text_cursor("a\u{7}\u{301}", "a\u{301}", 0, 1);
}

#[test]
fn settings_text_cursor_snaps_before_an_emoji_cluster_joined_by_stripping_ansi() {
    assert_projected_text_cursor("\u{2764}\x1b[0m\u{fe0f}", "\u{2764}\u{fe0f}", 0, 2);
}

#[test]
fn settings_text_cursor_preserves_normal_wide_grapheme_and_end_positions() {
    assert_projected_text_cursor("界Z", "界", 2, 3);
}

#[test]
fn settings_refuses_incomplete_geometry_and_delays_pending_copy() {
    let editor = editor_with(UserPreferencesValues::default());
    for (width, height) in [(35, 10), (36, 9), (1, 1)] {
        let (terminal, map, result) = render_once(
            &editor,
            width,
            height,
            Locale::EnUs,
            &Theme::terminal_native(),
        );
        assert_eq!(result, ModalPresentation::NotPresented);
        assert!(screen_text(&terminal).trim().is_empty());
        assert!((0..height).all(|y| (0..width).all(|x| map.hit_target_at(x, y).is_none())));
        assert!(!terminal.backend().cursor_visible());
    }

    let mut terminal = Terminal::new(TestBackend::new(35, 10)).unwrap();
    let mut hits = HitMapBuilder::new();
    let underlay = HitTarget::Help {
        page: 0,
        action: HelpControl::Close,
    };
    let underlay_rect = Rect::new(0, 0, 9, 1);
    let mut result =
        ModalPresentation::Presented(SettingsDraw::surface(editor.view().expect("draft").token()));
    terminal
        .draw(|frame| {
            frame.render_widget(Paragraph::new("underlay"), underlay_rect);
            frame.set_cursor_position((8, 0));
            hits.record(HitLayer::Foreground, underlay_rect, underlay.clone());
            result = render(
                frame,
                frame.area(),
                editor.view().expect("draft"),
                false,
                Locale::EnUs,
                &Theme::terminal_native(),
                &mut hits,
            );
        })
        .unwrap();
    assert_eq!(result, ModalPresentation::NotPresented);
    assert!(line_text(&terminal, 0).starts_with("underlay"));
    assert!(terminal.backend().cursor_visible());
    assert_eq!(
        terminal.get_cursor_position().unwrap(),
        ratatui::layout::Position::new(8, 0)
    );
    let map = hits.finish();
    assert_eq!(map.hit_target_at(0, 0), Some(&underlay));
    assert_eq!(map.hit_target_at(9, 0), None);

    let mut editor = Editor::default();
    editor.open().unwrap();
    for pending in [false, true] {
        let mut terminal = Terminal::new(TestBackend::new(58, 10)).unwrap();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    editor.view().unwrap(),
                    pending,
                    Locale::EnUs,
                    &Theme::terminal_native(),
                    &mut HitMapBuilder::new(),
                );
            })
            .unwrap();
        assert_eq!(screen_text(&terminal).contains("[..] Opening"), pending);
    }
}

#[test]
fn invalid_nondefault_executable_renders_its_profile_required_field_and_correction_in_both_locales()
{
    for locale in [Locale::EnUs, Locale::ZhCn] {
        for theme in [Theme::terminal_native(), Theme::one_light()] {
            let mut editor = editor_with(UserPreferencesValues::default());
            editor.edit(Edit::Select(Row::LaunchProfiles));
            editor.edit(Edit::Activate);
            editor.edit(Edit::Select(Row::AddProfile));
            editor.edit(Edit::Activate);
            editor.edit(Edit::Select(Row::ProfileName));
            editor.edit(Edit::Activate);
            editor.edit(Edit::Clear);
            editor.paste("Broken profile");
            editor.edit(Edit::Activate);
            editor.edit(Edit::Select(Row::CommandType));
            editor.edit(Edit::Activate);
            select_and_present(&mut editor, Row::Executable, locale, &theme);
            let View::Draft(field) = editor.view().unwrap() else {
                panic!("profile field")
            };
            assert_eq!(
                locale.settings_value(field, Row::Executable),
                match locale {
                    Locale::EnUs => "Required",
                    Locale::ZhCn => "必填",
                }
            );
            editor.edit(Edit::Cancel);
            select_and_present(&mut editor, Row::Save, locale, &theme);
            assert!(editor.edit(Edit::Activate).is_none());

            for (width, height) in [(120, 40), (80, 24), (58, 12), (36, 10)] {
                let (terminal, map, draw) =
                    present_once(&mut editor, width, height, locale, &theme);
                let View::Draft(draft) = editor.view().unwrap() else {
                    panic!("retained profile")
                };
                assert_eq!(draft.profile, 1);
                assert_eq!(draft.selected, Row::Executable);
                let rows = Page::Profiles.rows();
                let selected = rows.iter().position(|row| *row == Row::Executable).unwrap();
                assert!(
                    (draw.first_visible().unwrap()
                        ..draw.first_visible().unwrap() + draw.visible_rows().unwrap())
                        .contains(&selected)
                );
                assert!(
                    !cells_for(
                        &map,
                        &HitTarget::SettingsRow {
                            token: draft.token,
                            row: Row::Executable
                        },
                        width,
                        height
                    )
                    .is_empty()
                );
                let screen = without_cell_padding(&screen_text(&terminal));
                assert!(screen.contains(match locale {
                    Locale::EnUs => "Required",
                    Locale::ZhCn => "必填",
                }));
                if width >= 80 {
                    assert!(screen.contains(match locale {
                        Locale::EnUs => "Brokenprofile→Executable:absolutepathrequired",
                        Locale::ZhCn => "Brokenprofile→可执行文件：必填绝对路径",
                    }));
                    assert!(!screen.contains(match locale {
                        Locale::EnUs => "Invalidvalue;thedraftwaskept.",
                        Locale::ZhCn => "值无效；草稿已保留。",
                    }));
                }
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content
                        .iter()
                        .any(|cell| cell.symbol() == "!"
                            && cell.fg == theme.color(Role::StatusError))
                );
            }
            // A relative value is refused locally and keeps the same actionable feedback.
            editor.edit(Edit::Activate);
            editor.paste("relative-program");
            editor.edit(Edit::Activate);
            let (terminal, _, _) = present_once(&mut editor, 120, 40, locale, &theme);
            let screen = without_cell_padding(&screen_text(&terminal));
            assert!(screen.contains("relative-program"));
            assert!(screen.contains(match locale {
                Locale::EnUs => "absolutepathrequired",
                Locale::ZhCn => "必填绝对路径",
            }));
        }
    }
}
