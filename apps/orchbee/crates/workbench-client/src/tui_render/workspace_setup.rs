//! Bounded Project choices and text fields publish controls only after a complete draw.
use super::controls::{self, ActionKind, KeyHint};
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, FieldTone, ModalAction,
    NAME_FIELD_WIDTH, NameForm, dialog_layout, field_value_area, modal_key_label, paint_dialog,
    paint_field, paint_name_form, pointer_presentation,
};
use crate::client_view::{
    Binding, Bindings, ClientAction, KeyCode, KeyLabelGlyphs, Modifiers, PrefixAction,
};
use crate::localization::{DisplayPhase, FieldError, Locale, ShortcutLabel};
use crate::terminal_text::{
    display_width, sanitize_terminal_text, truncate_display, truncate_display_middle,
};
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ModalControl,
};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::{GlyphMode, Role, Theme};
use crate::workbench_state::workspace_setup::{
    LocationLabelStatus, SetupDraftView, SetupStage, SetupStatus, SetupToken,
};
use ratatui::{Frame, layout::Rect, text::Text, widgets::Paragraph};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DirectoryGuidance {
    fact: String,
    /// The action and its discovery title; a Workspace without a directory has none. Guidance
    /// appears only for a connected Empty workbench, so it never repeats the help table's
    /// connection qualifier.
    hint: Option<(PrefixAction, ShortcutLabel)>,
}

pub(super) fn directory_guidance(
    locale: Locale,
    phase: DisplayPhase,
    retired: bool,
    path: Option<&str>,
    unconfigured: bool,
) -> Option<DirectoryGuidance> {
    if retired || phase != DisplayPhase::Empty {
        return None;
    }
    path.map(|path| DirectoryGuidance {
        fact: locale.setup_directory_ready(&sanitize_terminal_text(path)),
        hint: Some((
            PrefixAction::Client(ClientAction::CreateTab),
            ShortcutLabel::NewTab,
        )),
    })
    .or_else(|| {
        unconfigured.then(|| DirectoryGuidance {
            fact: locale.setup_directory_needed().to_owned(),
            hint: None,
        })
    })
}

pub(super) fn render_directory_guidance(
    frame: &mut Frame<'_>,
    area: Rect,
    guidance: &DirectoryGuidance,
    bindings: &Bindings,
    locale: Locale,
    admitted: bool,
    theme: &Theme,
) {
    if area.is_empty() || frame.area().intersection(area) != area {
        return;
    }
    let fact = crate::terminal_text::wrap_text(&Text::from(guidance.fact.as_str()), area.width);
    let fact_rows = fact.lines.len();
    frame.render_widget(
        Paragraph::new(fact).style(theme.style(Role::TextSecondary)),
        area,
    );
    let (Some((action, label)), true) = (&guidance.hint, admitted) else {
        return;
    };
    let glyphs = match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    };
    let Some(key) = bindings
        .action_labels_with_glyphs(action.clone(), glyphs)
        .into_iter()
        .next()
    else {
        return;
    };
    let Ok(fact_rows) = u16::try_from(fact_rows) else {
        return;
    };
    if fact_rows >= area.height {
        return;
    }
    let Some(hint_y) = area.y.checked_add(fact_rows) else {
        return;
    };
    let hint_area = Rect::new(area.x, hint_y, area.width, 1);
    let label = locale.shortcut_label(*label);
    let _ = super::controls::render_key_hints(
        frame,
        hint_area,
        &[super::controls::KeyHint::new(&key, &label)],
        theme,
    );
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &SetupDraftView<'_>,
    status: Option<SetupStatus>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<SetupToken>) {
    match draft.stage() {
        SetupStage::QuickProject | SetupStage::ChooseWorkspaceProject => render_projects(
            frame,
            locale,
            &ProjectPicker {
                token: draft.token(),
                stage: draft.stage(),
                workspace: draft.workspace_label(),
                count: draft.project_count(),
                selected: draft.selected_project(),
            },
            theme,
            hits,
            |index| {
                draft.project_at(index).map(|project| ProjectChoice {
                    position: project.position(),
                    label: project.label().unwrap_or(if project.ungrouped() {
                        locale.setup_ungrouped()
                    } else {
                        locale.setup_new_project()
                    }),
                })
            },
        ),
        SetupStage::QuickName | SetupStage::QuickDirectory => {
            render_quick(frame, locale, draft, theme, hits)
        }
        // Preparation runs without a dialog; the form opens once it is ready.
        SetupStage::PreparingWorkspace => (Rect::default(), ModalPresentation::NotPresented),
        SetupStage::WorkspaceProjectName
        | SetupStage::TabGroupName
        | SetupStage::ProjectRename
        | SetupStage::WorkspaceRename => render_form(frame, locale, draft, status, theme, hits),
    }
}

/// The whole task's status row: a refusal, or an outcome the Client could not confirm.
fn status_row(status: Option<SetupStatus>, locale: Locale) -> Option<DialogRow<'static>> {
    let role = match status? {
        SetupStatus::NotApplied
        | SetupStatus::LaunchRequestTooLarge
        | SetupStatus::LocationLabel {
            status: LocationLabelStatus::NotApplied,
            ..
        } => Role::StatusError,
        SetupStatus::CommittedNeedRefresh
        | SetupStatus::OutcomeUnknown
        | SetupStatus::LocationLabel {
            status: LocationLabelStatus::CommittedNeedRefresh | LocationLabelStatus::OutcomeUnknown,
            ..
        } => Role::StatusWarning,
        _ => return None,
    };
    Some(DialogRow::Status(locale.setup_status(status?).into(), role))
}

/// A field's help row: its error once an edit or a refused submission exposed it, otherwise its
/// help, or a reserved blank row.
fn help_row<'a>(
    locale: Locale,
    error: Option<FieldError>,
    touched: bool,
    help: Option<&'a str>,
) -> DialogRow<'a> {
    match error.filter(|_| touched) {
        Some(error) => DialogRow::Error(locale.field_error(error).into()),
        None => DialogRow::Help(help.map(Into::into)),
    }
}

fn form_actions<'a>(
    cancel: &'a str,
    submit: &'a str,
    valid: bool,
    escape: &'a str,
    enter: &'a str,
) -> [ModalAction<'a>; 2] {
    [
        ModalAction::new(cancel, ActionKind::Secondary, true).with_shortcut(escape),
        ModalAction::new(submit, ActionKind::Primary, valid).with_shortcut(enter),
    ]
}

/// The caret sits inside the field's value cells.
fn place_caret(frame: &mut Frame<'_>, row: Rect, column: usize, theme: &Theme) {
    let value = field_value_area(row, theme);
    let column = u16::try_from(column)
        .unwrap_or(u16::MAX)
        .min(value.width.saturating_sub(1));
    frame.set_cursor_position((value.x.saturating_add(column), value.y));
}

/// The one-field forms: renames, a Tab group name, a Workspace directory, and the name of a new
/// Project for the Workspace being moved. A title that names the value omits the label.
fn render_form(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &SetupDraftView<'_>,
    status: Option<SetupStatus>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<SetupToken>) {
    let stage = draft.stage();
    let [error, _] = draft.shown_field_errors();
    let tab_group_rename = stage == SetupStage::TabGroupName && draft.renames_tab_group();
    let (title, submit) = if tab_group_rename {
        (
            locale.tab_group_rename_title(),
            locale.rename_submit_label(),
        )
    } else {
        (locale.setup_title(stage), locale.setup_submit_label(stage))
    };
    let help = match stage {
        SetupStage::TabGroupName => Some(locale.setup_group_name_hint()),
        _ => None,
    };
    let mut lead = Vec::new();
    // The Workspace the task acts on, when the title does not name it.
    if stage == SetupStage::WorkspaceProjectName
        && let Some(workspace) = draft.workspace_label()
    {
        lead.push(DialogRow::Target(
            locale.workspace_display_label(workspace).to_owned().into(),
        ));
        lead.push(DialogRow::Blank);
    }
    // A new Project's title does not name the value, so its field keeps the label.
    if stage == SetupStage::WorkspaceProjectName {
        lead.push(DialogRow::Label(
            locale.quick_workspace_name().into(),
            false,
        ));
    }
    let mut trailing = vec![help_row(locale, error, draft.touched(false), help)];
    trailing.extend(status_row(status, locale));
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = form_actions(
        locale.setup_cancel_label(),
        submit,
        error.is_none(),
        &escape,
        &enter,
    );
    let target = |control| HitTarget::Setup {
        token: draft.token(),
        control,
    };
    let form = NameForm {
        title,
        lead,
        field_minimum: NAME_FIELD_WIDTH,
        trailing,
        actions: &actions,
        placeholder: None,
    };
    let targets = DialogTargets {
        close: target(ModalControl::Close),
        actions: &[target(ModalControl::Cancel), target(ModalControl::Submit)],
        dismiss_outside: !draft.touched(false),
        scroll: None,
    };
    match paint_name_form(
        frame,
        form,
        |width| draft.field_window(width),
        &targets,
        theme,
        hits,
    ) {
        Some(area) => (area, ModalPresentation::Presented(draft.token())),
        None => (Rect::default(), ModalPresentation::NotPresented),
    }
}

struct ProjectPicker<'a> {
    token: SetupToken,
    stage: SetupStage,
    workspace: Option<&'a str>,
    count: usize,
    selected: usize,
}

struct ProjectChoice<'a> {
    position: usize,
    label: &'a str,
}

#[derive(Clone, Copy)]
struct PickerRowGeometry {
    row: Rect,
    navigation: Rect,
    primary: Rect,
}

impl PickerRowGeometry {
    fn resolve(row: Rect, theme: &Theme) -> Option<Self> {
        let navigation_width =
            u16::try_from(display_width(theme.glyphs.navigation_marker).max(1)).ok()?;
        let primary_x = row.x.checked_add(navigation_width)?.checked_add(1)?;
        let primary = Rect::new(
            primary_x,
            row.y,
            row.right().checked_sub(primary_x)?,
            row.height,
        );
        let navigation = Rect::new(row.x, row.y, navigation_width, row.height);
        (!row.is_empty()
            && !primary.is_empty()
            && row.intersection(navigation) == navigation
            && row.intersection(primary) == primary)
            .then_some(Self {
                row,
                navigation,
                primary,
            })
    }

    fn prefix_width(self) -> usize {
        usize::from(self.primary.x.saturating_sub(self.row.x))
    }
}

/// The Project pickers: moving a Workspace, and the first step of a new Workspace. A picker has
/// a hint row instead of buttons, and a press outside closes it.
fn render_projects<'a>(
    frame: &mut Frame<'_>,
    locale: Locale,
    picker: &ProjectPicker<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    choice: impl Fn(usize) -> Option<ProjectChoice<'a>>,
) -> (Rect, ModalPresentation<SetupToken>) {
    let count = picker.count;
    if count == 0 {
        return (Rect::default(), ModalPresentation::NotPresented);
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let up = modal_key_label(theme, &Binding::new(KeyCode::Up, Modifiers::NONE));
    let down = modal_key_label(theme, &Binding::new(KeyCode::Down, Modifiers::NONE));
    let vertical = format!("{up}{down}");
    let hints = [
        KeyHint::new(&escape, locale.setup_close_hint()),
        KeyHint::new(&enter, locale.setup_choose_hint(picker.stage)),
        KeyHint::new(&vertical, locale.setup_select_hint()),
    ];
    let mut rows = Vec::new();
    if picker.stage == SetupStage::ChooseWorkspaceProject
        && let Some(workspace) = picker.workspace
    {
        rows.push(DialogRow::Target(
            locale.workspace_display_label(workspace).to_owned().into(),
        ));
        rows.push(DialogRow::Blank);
    }
    let list = rows.len();
    rows.push(DialogRow::List { len: count });
    let dialog = Dialog {
        title: locale.setup_title(picker.stage),
        class: DialogClass::Medium,
        rows,
        footer: DialogFooter::Hints(&hints),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(list_area) = layout.row(list) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let capacity = usize::from(list_area.height);
    let start = picker
        .selected
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    let geometries = (0..capacity.min(count - start))
        .map(|offset| {
            let y = list_area.y.checked_add(u16::try_from(offset).ok()?)?;
            PickerRowGeometry::resolve(Rect::new(list_area.x, y, list_area.width, 1), theme)
        })
        .collect::<Option<Vec<_>>>();
    let Some(geometries) = geometries else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    // A row whose Project is missing stays blank and takes no click target.
    let rows = geometries
        .into_iter()
        .enumerate()
        .filter_map(|(offset, geometry)| {
            let project = choice(start + offset)?;
            let prefix = format!("{}. ", project.position);
            let label = format!(
                "{prefix}{}",
                truncate_display(
                    &sanitize_terminal_text(project.label),
                    usize::from(geometry.primary.width).saturating_sub(display_width(&prefix)),
                    theme.glyphs.ellipsis,
                )
            );
            Some((start + offset, geometry, label))
        })
        .collect::<Vec<_>>();
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: HitTarget::Setup {
                token: picker.token,
                control: ModalControl::Close,
            },
            actions: &[],
            dismiss_outside: true,
            scroll: Some(HitTarget::ListScroll(
                crate::tui_hit_map::ListScrollTarget::Setup(picker.token),
            )),
        },
        theme,
        hits,
    );
    for (index, geometry, label) in rows {
        frame.render_widget(
            Paragraph::new(format!("{}{label}", " ".repeat(geometry.prefix_width())))
                .style(theme.style(Role::TextPrimary)),
            geometry.row,
        );
        let target = HitTarget::Setup {
            token: picker.token,
            control: ModalControl::Choice(index),
        };
        let availability = InteractionAvailability::Enabled;
        let pointer =
            hits.projected_modal_pointer(HitLayer::Overlay, geometry.row, &target, availability);
        let provisional = index == picker.selected;
        let state = controls::InteractionState::new(
            if provisional {
                controls::FocusState::Focused
            } else {
                controls::FocusState::Unfocused
            },
            pointer_presentation(pointer),
            if provisional {
                controls::NavigationState::Provisional
            } else {
                controls::NavigationState::None
            },
            controls::CurrentState::NotCurrent,
            controls::CheckedState::Unchecked,
            controls::Availability::Enabled,
            controls::OperationState::Idle,
        );
        let painted = controls::paint_row_interaction(
            frame,
            geometry.row,
            controls::RowTracks::new(controls::RowFocusCue::EnclosingFrame, geometry.primary)
                .with_provisional(geometry.navigation),
            &[],
            controls::ControlSurface::Overlay,
            state,
            theme,
        )
        .expect("admitted setup picker row must paint");
        assert_eq!(painted, geometry.row);
        hits.record_modal_control(HitLayer::Overlay, geometry.row, target, availability);
    }
    (layout.area, ModalPresentation::Presented(picker.token))
}

/// The New Workspace form: a name, a directory, and the recent folders under the directory.
fn render_quick(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &SetupDraftView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<SetupToken>) {
    const NAME: usize = 3;
    const DIRECTORY: usize = 7;
    const HEADING: usize = 9;
    const CHOICES: usize = 10;
    let class = DialogClass::Medium;
    let project = draft.project_label().unwrap_or(locale.setup_ungrouped());
    let directory_active = draft.stage() == SetupStage::QuickDirectory;
    let errors = draft.shown_field_errors();
    let count = draft.directory_choices().len();
    // The directory's empty help row separates the field from its list; an error takes its place.
    let mut rows = vec![
        DialogRow::Context(locale.setup_project_context(project).into()),
        DialogRow::Blank,
        DialogRow::Label(locale.quick_workspace_name().into(), !directory_active),
        DialogRow::Field {
            minimum: NAME_FIELD_WIDTH,
        },
        help_row(locale, errors[0], draft.touched(false), None),
        DialogRow::Blank,
        DialogRow::Label(locale.quick_workspace_directory().into(), directory_active),
        DialogRow::Field {
            minimum: class.path_width(),
        },
        help_row(locale, errors[1], draft.touched(true), None),
    ];
    if count > 0 {
        rows.push(DialogRow::Heading(locale.quick_workspace_recent().into()));
        rows.push(DialogRow::List {
            len: count.min(RECENT_ROWS),
        });
    }
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = form_actions(
        locale.setup_cancel_label(),
        locale.setup_submit_label(draft.stage()),
        errors.iter().all(Option::is_none),
        &escape,
        &enter,
    );
    let dialog = Dialog {
        title: locale.setup_title(draft.stage()),
        class,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let (Some(name_row), Some(directory_row)) = (layout.row(NAME), layout.row(DIRECTORY)) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let width = usize::from(field_value_area(name_row, theme).width);
    let (Some(name), Some(directory)) = (
        draft.quick_window(false, width),
        draft.quick_window(true, width),
    ) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    // The list keeps the selected folder in view; rows that do not fit scroll with it.
    let list = layout.row(CHOICES);
    let capacity = list.map_or(0, |list| usize::from(list.height));
    let start = draft
        .selected_directory()
        .unwrap_or(0)
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    let folders = list.map_or(Some(Vec::new()), |list| {
        (start..count.min(start + capacity))
            .map(|index| {
                let y = list.y.checked_add(u16::try_from(index - start).ok()?)?;
                let geometry =
                    PickerRowGeometry::resolve(Rect::new(list.x, y, list.width, 1), theme)?;
                let path = draft.directory_choices().get(index)?.as_str();
                Some((index, FolderRow::resolve(geometry, path, draft, theme)))
            })
            .collect::<Option<Vec<_>>>()
    });
    let Some(folders) = folders else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let target = |control| HitTarget::Setup {
        token: draft.token(),
        control,
    };
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: target(ModalControl::Close),
            actions: &[target(ModalControl::Cancel), target(ModalControl::Submit)],
            dismiss_outside: !draft.touched(false) && !draft.touched(true),
            scroll: None,
        },
        theme,
        hits,
    );
    // The focused value scrolls with its caret; at rest a name truncates at its end and a path
    // in its middle.
    let ellipsis = theme.glyphs.ellipsis;
    let name_value = if directory_active {
        truncate_display(
            &sanitize_terminal_text(draft.quick_text(false)),
            width,
            ellipsis,
        )
    } else {
        name.display().to_owned()
    };
    let directory_value = if directory_active {
        directory.display().to_owned()
    } else {
        truncate_display_middle(
            &sanitize_terminal_text(draft.quick_text(true)),
            width,
            ellipsis,
        )
    };
    paint_field(frame, name_row, &name_value, FieldTone::Value, theme);
    paint_field(
        frame,
        directory_row,
        &directory_value,
        FieldTone::Value,
        theme,
    );
    // A press on the other field moves the caret there.
    hits.record_modal_control(
        HitLayer::Overlay,
        if directory_active {
            name_row
        } else {
            directory_row
        },
        target(ModalControl::NextField),
        InteractionAvailability::Enabled,
    );
    // A clipped list shows which folders are in view, as `2-4/8`, at the heading's end.
    if let Some(heading) = layout.row(HEADING)
        && capacity < count
    {
        let range = format!("{}-{}/{count}", start + 1, start + folders.len());
        let range_width = display_width(&range);
        let used = display_width(locale.quick_workspace_recent()) + 1;
        if let Ok(cells) = u16::try_from(range_width)
            && used + range_width <= usize::from(heading.width)
        {
            frame.render_widget(
                Paragraph::new(range).style(theme.style(Role::TextSecondary)),
                Rect::new(heading.right() - cells, heading.y, cells, 1),
            );
        }
    }
    for (index, folder) in folders {
        folder.paint(frame, theme);
        let control = target(ModalControl::Choice(index));
        let availability = InteractionAvailability::Enabled;
        let row = folder.geometry.row;
        let pointer = hits.projected_modal_pointer(HitLayer::Overlay, row, &control, availability);
        let provisional = draft.selected_directory() == Some(index);
        let state = controls::InteractionState::new(
            controls::FocusState::Unfocused,
            pointer_presentation(pointer),
            if provisional {
                controls::NavigationState::Provisional
            } else {
                controls::NavigationState::None
            },
            controls::CurrentState::NotCurrent,
            controls::CheckedState::Unchecked,
            controls::Availability::Enabled,
            controls::OperationState::Idle,
        );
        let bands = folder
            .parent
            .as_ref()
            .map(|(rect, _)| controls::RowContentBand::semantic(*rect, Role::TextSecondary));
        let painted = controls::paint_row_interaction(
            frame,
            row,
            controls::RowTracks::new(controls::RowFocusCue::EnclosingFrame, folder.name.0)
                .with_provisional(folder.geometry.navigation),
            bands.as_slice(),
            controls::ControlSurface::Overlay,
            state,
            theme,
        )
        .expect("admitted recent folder row must paint");
        assert_eq!(painted, row);
        hits.record_modal_control(HitLayer::Overlay, row, control, availability);
    }
    let (row, window) = if directory_active {
        (directory_row, &directory)
    } else {
        (name_row, &name)
    };
    place_caret(frame, row, window.cursor_column(), theme);
    (layout.area, ModalPresentation::Presented(draft.token()))
}

/// The recent folders the New Workspace form lists at most; more scroll with the selection.
const RECENT_ROWS: usize = 3;
/// Cells between a folder's name and its parent.
const FOLDER_GAP: usize = 2;
/// The fewest cells a cut parent keeps; with less room the parent goes and the name stays whole.
const PARENT_MINIMUM: usize = 8;

/// One recent folder: its name in the primary track and, as right-aligned metadata, the folder
/// that holds it, spelled from `~` inside the home folder.
struct FolderRow {
    geometry: PickerRowGeometry,
    name: (Rect, String),
    parent: Option<(Rect, String)>,
}

impl FolderRow {
    fn resolve(
        geometry: PickerRowGeometry,
        path: &str,
        draft: &SetupDraftView<'_>,
        theme: &Theme,
    ) -> Self {
        let ellipsis = theme.glyphs.ellipsis;
        let primary = geometry.primary;
        let cells = usize::from(primary.width);
        let (name, parent) = folder_parts(path);
        let name = sanitize_terminal_text(name);
        let name_width = display_width(&name);
        let parent = parent.and_then(|parent| {
            let parent = draft
                .home_spelling(parent)
                .unwrap_or_else(|| parent.to_owned());
            let parent = sanitize_terminal_text(&parent).into_owned();
            let room = cells.checked_sub(name_width + FOLDER_GAP)?;
            if room < display_width(&parent).min(PARENT_MINIMUM) {
                return None;
            }
            let parent = truncate_display_middle(&parent, room, ellipsis);
            let width = u16::try_from(display_width(&parent)).ok()?;
            let rect = Rect::new(primary.right() - width, primary.y, width, 1);
            Some((rect, parent))
        });
        let name_rect = parent.as_ref().map_or(primary, |(rect, _)| {
            Rect::new(primary.x, primary.y, rect.x - primary.x, 1)
        });
        Self {
            geometry,
            name: (name_rect, truncate_display(&name, cells, ellipsis)),
            parent,
        }
    }

    fn paint(&self, frame: &mut Frame<'_>, theme: &Theme) {
        frame.render_widget(
            Paragraph::new(self.name.1.as_str()).style(theme.style(Role::TextPrimary)),
            self.name.0,
        );
        if let Some((rect, parent)) = &self.parent {
            frame.render_widget(
                Paragraph::new(parent.as_str()).style(theme.style(Role::TextSecondary)),
                *rect,
            );
        }
    }
}

/// A folder path's last component and the folder that holds it; a drive root keeps its
/// separator, and a root has no parent.
fn folder_parts(path: &str) -> (&str, Option<&str>) {
    let trimmed = path.trim_end_matches(['\\', '/']);
    match trimmed.rfind(['\\', '/']) {
        Some(at) if at + 1 < trimmed.len() => {
            let parent = &trimmed[..at];
            let parent = if parent.is_empty() || parent.ends_with(':') {
                &trimmed[..=at]
            } else {
                parent
            };
            (&trimmed[at + 1..], Some(parent))
        }
        _ => (path, None),
    }
}

#[cfg(test)]
mod tests;
