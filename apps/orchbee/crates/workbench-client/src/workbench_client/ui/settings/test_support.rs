//! Whole-frame preview, restoration and confirmed publication across the native painters.
//!
//! The Client state and every assertion stay here; the owning application paints each frame
//! through its real shell drawing boundary.
use super::{Edit, Row, WorkbenchClientUi, WorkbenchClientView};
use crate::{
    client_view::ClientView,
    localization::Locale,
    native_settings::Control,
    tui_theme::{ColorLevel, Role},
    workbench_client::DrawReceipt,
};
use orchbee_workbench_protocol::{
    ApplicationAppearance, SettingChange, SettingScalar, SettingValue, SettingsFile,
    SettingsLocale, SettingsPlatform, SettingsSnapshot, TerminalDisplayAppearance,
    UserSettingsValues, WorkbenchRevision,
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

/// The snapshot the Server publishes at `revision` for a file that gives `values`.
pub(in crate::workbench_client) fn settings_snapshot(
    revision: u64,
    values: &UserSettingsValues,
) -> SettingsSnapshot {
    let changes = orchbee_local_runtime::value_changes(&UserSettingsValues::default(), values, &[]);
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

fn light() -> UserSettingsValues {
    UserSettingsValues {
        application_theme: ApplicationAppearance::Light,
        ..UserSettingsValues::default()
    }
}

fn ready_with(values: &UserSettingsValues) -> WorkbenchClientUi {
    let (mut ui, _, _, _) = super::super::test_support::ready(true);
    let token = ui.settings.editor.open().unwrap();
    assert!(
        ui.settings
            .editor
            .loaded(token, &settings_snapshot(1, values))
    );
    ui.apply_user_preferences(values);
    draw(&mut ui);
    ui
}

fn draw(ui: &mut WorkbenchClientUi) {
    ui.settings
        .editor
        .draw(ui.settings.editor.view().unwrap().token());
}

/// Focuses a choice row and opens its list, so its candidates preview.
fn open_list(ui: &mut WorkbenchClientUi, row: Row) {
    ui.settings.editor.edit(Edit::Select(row));
    draw(ui);
    ui.settings.editor.edit(Edit::Activate);
    draw(ui);
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

fn text(buffer: &Buffer) -> String {
    buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>()
        .replace(' ', "")
}

/// Scenario body for the owning application's test of the same name.
pub fn a_candidate_previews_on_the_whole_page_without_runtime_effects(
    paint_frame: SettingsFrame<'_>,
) {
    for size in [(120, 40), (50, 20)] {
        for locale in [Locale::EnUs, Locale::ZhCn] {
            for level in [
                ColorLevel::Truecolor,
                ColorLevel::Ansi256,
                ColorLevel::Ansi16,
                ColorLevel::Monochrome,
            ] {
                let values = light();
                let mut ui = ready_with(&values);
                let mut shell = ClientView::new(size.0);
                shell.configure_color_level(level);
                shell.apply_user_preferences(values.clone());
                let area = ratatui::layout::Rect::new(0, 0, size.0, size.1);
                let areas = crate::tui_layout::project_shell_areas(area, shell.sidebar_surface());
                ui.settings
                    .editor
                    .control(ui.settings.editor.view().unwrap().token(), Control::Close);
                let original = paint(&mut ui, &mut shell, size, locale, paint_frame);
                let token = ui.settings.editor.open().unwrap();
                assert!(
                    ui.settings
                        .editor
                        .loaded(token, &settings_snapshot(1, &values))
                );
                paint(&mut ui, &mut shell, size, locale, paint_frame);
                let confirmed_shell = shell.clone();
                open_list(&mut ui, Row::TerminalTheme);
                for (edit, appearance) in [
                    (Edit::Next, TerminalDisplayAppearance::Host),
                    (Edit::Next, TerminalDisplayAppearance::Custom),
                    (Edit::Previous, TerminalDisplayAppearance::Host),
                ] {
                    ui.settings.editor.edit(edit);
                    let preview = ui.view().settings_appearance_preview().unwrap();
                    assert_eq!(preview.terminal_display_theme, appearance);
                    let expected = shell.preview_theme(preview);
                    let buffer = paint(&mut ui, &mut shell, size, locale, paint_frame);
                    assert_eq!(
                        buffer[(0, size.1 - 1)].bg,
                        expected.color(Role::SurfaceCanvas),
                        "the candidate reaches the page's status row at {size:?}, {level:?}"
                    );
                    assert!(text(&buffer).contains(locale.settings_title()) || size.0 < 56);
                    assert_eq!(shell, confirmed_shell);
                    assert_eq!(ui.preferences, values);
                    assert!(ui.take_settings_snapshot().is_none());
                    assert_eq!(
                        crate::tui_layout::project_shell_areas(area, shell.sidebar_surface()),
                        areas
                    );
                }
                // Esc closes the list and drops its candidate; closing the page restores the shell.
                ui.settings.editor.edit(Edit::Cancel);
                assert_eq!(
                    ui.view()
                        .settings_appearance_preview()
                        .unwrap()
                        .terminal_display_theme,
                    values.terminal_display_theme
                );
                ui.settings
                    .editor
                    .control(ui.settings.editor.view().unwrap().token(), Control::Close);
                let restored = paint(&mut ui, &mut shell, size, locale, paint_frame);
                assert_eq!(restored[(0, 0)], original[(0, 0)]);
                assert_eq!(shell, confirmed_shell);
            }
        }
    }
}

/// Scenario body for the owning application's test of the same name.
pub fn a_change_made_elsewhere_shows_on_the_open_page_at_once(paint_frame: SettingsFrame<'_>) {
    let initial = light();
    let mut ui = ready_with(&initial);
    let mut shell = ClientView::new(120);
    shell.apply_user_preferences(initial);
    let latest = UserSettingsValues {
        application_theme: ApplicationAppearance::Dark,
        locale: SettingsLocale::ZhCn,
        ..UserSettingsValues::default()
    };
    assert!(ui.settings.observe_snapshot(settings_snapshot(2, &latest)));
    let confirmed = orchbee_local_runtime::snapshot_values(&ui.take_settings_snapshot().unwrap());
    ui.apply_user_preferences(&confirmed);
    shell.apply_user_preferences(confirmed);
    let preview = ui.view().settings_appearance_preview().unwrap();
    assert_eq!(preview.locale, SettingsLocale::ZhCn);
    assert_eq!(preview.application_theme, ApplicationAppearance::Dark);
    let buffer = paint(&mut ui, &mut shell, (120, 40), Locale::EnUs, paint_frame);
    assert_eq!(
        buffer[(0, 0)].bg,
        shell.configured_theme().unwrap().color(Role::SurfaceChrome)
    );
    assert!(text(&buffer).contains(Locale::ZhCn.settings_title()));
    assert!(ui.settings.editor.visible(), "the page stays open");
}

/// Scenario body for the owning application's test of the same name.
pub fn covering_client_owner_uses_confirmed_appearance_until_settings_returns(
    paint_frame: SettingsFrame<'_>,
) {
    let mut ui = ready_with(&UserSettingsValues::default());
    let mut shell = ClientView::new(120);
    // A named theme, which the previewed Light appearance paints, needs truecolor.
    shell.configure_color_level(crate::tui_theme::ColorLevel::Truecolor);
    shell.apply_user_preferences(UserSettingsValues::default());
    open_list(&mut ui, Row::Theme);
    ui.settings.editor.edit(Edit::Next);
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
