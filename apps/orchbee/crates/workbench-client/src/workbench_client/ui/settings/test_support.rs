//! Whole-frame preview, restoration and confirmed publication across the native painters.
//!
//! The Client state and every assertion stay here; the owning application paints each frame
//! through its real shell drawing boundary.
use super::{Edit, Row, View, WorkbenchClientUi, WorkbenchClientView};
use crate::{
    client_view::ClientView,
    localization::Locale,
    native_settings::Basis,
    tui_theme::{ColorLevel, Role},
    workbench_client::DrawReceipt,
};
use orchbee_workbench_protocol::{
    ApplicationAppearance, PreferencesLocale, SettingChange, SettingScalar, SettingValue,
    SettingsFile, SettingsPlatform, SettingsSnapshot, TerminalDisplayAppearance,
    UserPreferencesValues, WorkbenchRevision,
};
use ratatui::buffer::Buffer;

/// The Client view a whole-frame Settings paint draws.
pub trait SettingsFrameSource {
    fn settings_view(&self) -> WorkbenchClientView<'_>;
}

/// One whole native frame painted by the owning application's shell, with its draw receipt.
pub(crate) type SettingsFrame<'a> = &'a dyn Fn(
    (u16, u16),
    &mut ClientView,
    Locale,
    &dyn SettingsFrameSource,
) -> (Buffer, DrawReceipt);

impl SettingsFrameSource for WorkbenchClientUi {
    fn settings_view(&self) -> WorkbenchClientView<'_> {
        self.view()
    }
}

pub(super) fn ready() -> WorkbenchClientUi {
    ready_with_basis(draft_basis())
}

pub(super) fn draft_basis() -> Basis {
    Basis {
        revision: 1,
        values: orchbee_workbench_protocol::UserPreferencesValues::default(),
    }
}

/// The snapshot the Server publishes at `revision` for a file that gives `values`.
pub(super) fn settings_snapshot(revision: u64, values: &UserPreferencesValues) -> SettingsSnapshot {
    let changes =
        orchbee_local_runtime::preference_changes(&UserPreferencesValues::default(), values, &[]);
    SettingsSnapshot {
        revision: WorkbenchRevision::new(revision),
        file_path: None,
        file: SettingsFile::Ok {},
        platform: if cfg!(windows) {
            SettingsPlatform::Windows
        } else if cfg!(target_os = "macos") {
            SettingsPlatform::Macos
        } else {
            SettingsPlatform::Linux
        },
        values: changes
            .into_iter()
            .filter_map(|change| match change {
                SettingChange::Set { key, value } => Some((
                    key,
                    Some(match value {
                        SettingValue::Boolean(value) => SettingScalar::Boolean(value),
                        SettingValue::Integer(value) => SettingScalar::Integer(value),
                        SettingValue::Text(value) => SettingScalar::Text(value),
                        SettingValue::Entries(_) => return None,
                    }),
                )),
                SettingChange::Restore { .. } => None,
            })
            .collect(),
        set: Vec::new(),
        launch_profiles: values.launch_profiles.clone(),
        diagnostics: Default::default(),
    }
}

pub(super) fn ready_with_application_theme(
    application_theme: orchbee_workbench_protocol::ApplicationAppearance,
) -> WorkbenchClientUi {
    ready_with_basis(draft_basis_with_application_theme(application_theme))
}

pub(super) fn ready_with_basis(basis: Basis) -> WorkbenchClientUi {
    let (mut ui, _, _, _) = super::super::test_support::ready(true);
    let token = ui.settings.editor.open().unwrap();
    assert!(ui.settings.editor.loaded(token, basis.clone()));
    ui.apply_user_preferences(&basis.values);
    ui.settings
        .editor
        .draw(ui.settings.editor.view().unwrap().token());
    ui
}

pub(super) fn draft_basis_with_application_theme(
    application_theme: orchbee_workbench_protocol::ApplicationAppearance,
) -> Basis {
    Basis {
        revision: 1,
        values: orchbee_workbench_protocol::UserPreferencesValues {
            application_theme,
            ..orchbee_workbench_protocol::UserPreferencesValues::default()
        },
    }
}

fn paint(
    ui: &mut WorkbenchClientUi,
    shell: &mut ClientView,
    size: (u16, u16),
    locale: Locale,
    paint_frame: SettingsFrame<'_>,
) -> Buffer {
    let (buffer, receipt) = paint_frame(size, shell, locale, &*ui);
    ui.after_close_draw(receipt);
    buffer
}

/// Scenario body for the owning application's test of the same name.
pub fn draft_appearance_reaches_background_chrome_and_program_defaults_without_runtime_effects(
    paint_frame: SettingsFrame<'_>,
) {
    for size in [(120, 40), (80, 24)] {
        for fallback in [Locale::EnUs, Locale::ZhCn] {
            for level in [
                ColorLevel::Truecolor,
                ColorLevel::Ansi256,
                ColorLevel::Ansi16,
                ColorLevel::Monochrome,
            ] {
                let values = UserPreferencesValues {
                    application_theme: ApplicationAppearance::Light,
                    ..UserPreferencesValues::default()
                };
                let mut ui = ready_with_application_theme(ApplicationAppearance::Light);
                let mut shell = ClientView::new(size.0);
                shell.configure_color_level(level);
                shell.apply_user_preferences(values.clone());
                ui.apply_user_preferences(&values);
                let area = ratatui::layout::Rect::new(0, 0, size.0, size.1);
                let areas = crate::tui_layout::project_shell_areas(area, shell.sidebar_surface());
                let pane = crate::tui_layout::arranged_pane_area(&areas, &ui.view()).unwrap();
                let key = super::super::test_support::install_projection(&mut ui, None, 0);
                let projection = ui.projections.iter_mut().find(|p| p.key == key).unwrap();
                let mut frame =
                    super::super::test_support::frame(&projection.value, 0, false, pane.height);
                frame.geometry.columns = pane.width;
                frame.geometry.rows = pane.height;
                frame.foreground = 0x102030;
                frame.background = 0xe0d0c0;
                frame.cursor.visible = false;
                let row = &mut frame.rows[0];
                row.cells[0].text = "d".into();
                row.cells[0].style.foreground = 0x1000000;
                row.cells[0].style.background = 0x1000000;
                row.cells
                    .resize(usize::from(pane.width), row.cells[0].clone());
                let mut explicit = row.cells[0].clone();
                explicit.text = "e".into();
                explicit.style.foreground = 0xabcdef;
                explicit.style.background = 0x123456;
                row.cells[usize::from(pane.width) - 1] = explicit;
                projection.value.install_frame_for_test(frame);

                ui.settings.editor.edit(Edit::Cancel);
                let original = paint(&mut ui, &mut shell, size, fallback, paint_frame);
                assert_eq!(
                    original[(pane.right() - 2, pane.y)].symbol(),
                    "d",
                    "confirmed program baseline at {size:?}, {fallback:?}, {level:?}; pane={pane:?}"
                );
                let token = ui.settings.editor.open().unwrap();
                assert!(ui.settings.editor.loaded(
                    token,
                    draft_basis_with_application_theme(ApplicationAppearance::Light)
                ));
                paint(&mut ui, &mut shell, size, fallback, paint_frame);
                let confirmed_shell = shell.clone();
                ui.settings.editor.edit(Edit::Select(Row::Theme));
                ui.settings.editor.edit(Edit::Adjust(true));
                ui.settings.editor.edit(Edit::Select(Row::Language));
                ui.settings
                    .editor
                    .edit(Edit::Adjust(fallback == Locale::ZhCn));
                ui.settings.editor.edit(Edit::Select(Row::CopyOnSelect));
                ui.settings.editor.edit(Edit::Adjust(true));
                ui.settings.editor.edit(Edit::Select(Row::Sound));
                ui.settings.editor.edit(Edit::Adjust(true));
                ui.settings.editor.edit(Edit::Select(Row::TerminalTheme));
                for appearance in [
                    TerminalDisplayAppearance::Host,
                    TerminalDisplayAppearance::Custom,
                    TerminalDisplayAppearance::Theme,
                ] {
                    ui.settings.editor.edit(Edit::Adjust(true));
                    let preview = ui.view().settings_appearance_preview().unwrap();
                    assert_eq!(preview.terminal_display_theme, appearance);
                    let expected = shell
                        .preview_theme(preview)
                        .with_host_terminal_colors(0x102030, 0xe0d0c0);
                    let buffer = paint(&mut ui, &mut shell, size, fallback, paint_frame);
                    // Settings takes the whole host: at compact sizes its own surface covers the
                    // corner; at full size the sidebar chrome stays visible beside it.
                    let corner = if size.1 <= 30 {
                        Role::SurfaceOverlay
                    } else {
                        Role::SurfaceChrome
                    };
                    assert_eq!(
                        buffer[(0, size.1 - 1)].bg,
                        expected.color(corner),
                        "the draft reaches the bottom-left surface at {size:?}"
                    );
                    let defaults = expected.terminal_defaults();
                    if size.0 == 120 {
                        assert_eq!(
                            buffer[(pane.right() - 2, pane.y)].symbol(),
                            "d",
                            "preview program at {size:?}, {fallback:?}, {level:?}, {appearance:?}; pane={pane:?}"
                        );
                        assert_eq!(buffer[(pane.right() - 2, pane.y)].fg, defaults.0);
                        assert_eq!(buffer[(pane.right() - 2, pane.y)].bg, defaults.1);
                        assert_eq!(buffer[(pane.right() - 1, pane.y)].symbol(), "e");
                        assert_eq!(
                            buffer[(pane.right() - 1, pane.y)].fg,
                            ratatui::style::Color::Rgb(171, 205, 239)
                        );
                        assert_eq!(
                            buffer[(pane.right() - 1, pane.y)].bg,
                            ratatui::style::Color::Rgb(18, 52, 86)
                        );
                    } else {
                        // At 80x24 Settings covers this program row. Chrome and
                        // draft checks still apply; program colors require exposure.
                        assert_ne!(buffer[(pane.right() - 2, pane.y)].symbol(), "d");
                        assert_ne!(buffer[(pane.right() - 1, pane.y)].symbol(), "e");
                    }
                    let preview_locale = if fallback == Locale::EnUs {
                        Locale::ZhCn
                    } else {
                        Locale::EnUs
                    };
                    assert!(
                        buffer
                            .content
                            .iter()
                            .map(|cell| cell.symbol())
                            .collect::<String>()
                            .replace(' ', "")
                            .contains(preview_locale.settings_title())
                    );
                    assert_eq!(shell, confirmed_shell);
                    assert_eq!(ui.preferences, values);
                    assert!(ui.take_settings_snapshot().is_none());
                    assert_eq!(
                        crate::tui_layout::project_shell_areas(area, shell.sidebar_surface()),
                        areas
                    );
                }
                ui.settings.editor.edit(Edit::Cancel);
                let restored = paint(&mut ui, &mut shell, size, fallback, paint_frame);
                assert_eq!(restored[(0, 0)], original[(0, 0)]);
                assert_eq!(
                    restored[(pane.right() - 2, pane.y)],
                    original[(pane.right() - 2, pane.y)]
                );
                assert_eq!(shell, confirmed_shell);
            }
        }
    }
}

/// Scenario body for the owning application's test of the same name.
pub fn external_confirmed_values_update_unedited_presentation_and_cancel_reveals_the_latest_snapshot(
    paint_frame: SettingsFrame<'_>,
) {
    let mut ui = ready_with_application_theme(ApplicationAppearance::Light);
    let mut shell = ClientView::new(120);
    let initial = UserPreferencesValues {
        application_theme: ApplicationAppearance::Light,
        ..UserPreferencesValues::default()
    };
    shell.apply_user_preferences(initial.clone());
    ui.settings.editor.edit(Edit::Select(Row::Theme));
    ui.settings.editor.edit(Edit::Adjust(true));
    let mut latest = draft_basis().values;
    latest.application_theme = ApplicationAppearance::System;
    latest.locale = PreferencesLocale::ZhCn;
    assert!(
        ui.settings
            .observe_snapshot(settings_snapshot(2, &latest), ui.clock.now())
    );
    let confirmed =
        orchbee_local_runtime::snapshot_preferences(&ui.take_settings_snapshot().unwrap());
    ui.apply_user_preferences(&confirmed);
    shell.apply_user_preferences(confirmed);
    let preview = ui.view().settings_appearance_preview().unwrap();
    assert_eq!(preview.locale, PreferencesLocale::ZhCn);
    assert_eq!(preview.application_theme, ApplicationAppearance::Dark);
    let expected = shell.preview_theme(preview);
    let buffer = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs, paint_frame);
    assert_eq!(buffer[(0, 0)].bg, expected.color(Role::SurfaceChrome));
    assert!(
        buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .replace(' ', "")
            .contains(Locale::ZhCn.settings_title())
    );
    assert!(matches!(ui.settings.editor.view(), Some(View::Draft(draft))
        if draft.conflict && draft.values.application_theme == ApplicationAppearance::Dark
            && draft.basis_values == &initial));
    ui.settings.editor.edit(Edit::Cancel);
    assert_eq!(shell.user_preferences(), Some(&latest));
    let restored = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs, paint_frame);
    assert_eq!(
        restored[(0, 0)].bg,
        shell.configured_theme().unwrap().color(Role::SurfaceChrome)
    );
}

/// Scenario body for the owning application's test of the same name.
pub fn covering_client_owner_uses_confirmed_appearance_until_settings_returns(
    paint_frame: SettingsFrame<'_>,
) {
    let mut ui = ready();
    let mut shell = ClientView::new(120);
    // A named theme, which the previewed Light appearance paints, needs truecolor.
    shell.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
    shell.apply_user_preferences(UserPreferencesValues::default());
    ui.settings.editor.edit(Edit::Select(Row::Theme));
    ui.settings.editor.edit(Edit::Adjust(true));
    let preview = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs, paint_frame);
    shell.machines.open = true;
    let covered = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs, paint_frame);
    assert_eq!(
        covered[(0, 0)].bg,
        shell.configured_theme().unwrap().color(Role::SurfaceChrome)
    );
    assert_ne!(preview[(0, 0)].bg, covered[(0, 0)].bg);
    shell.machines.open = false;
    let resumed = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs, paint_frame);
    assert_eq!(resumed[(0, 0)].bg, preview[(0, 0)].bg);
}
