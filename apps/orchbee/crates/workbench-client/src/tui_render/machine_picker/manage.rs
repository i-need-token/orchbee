//! Painters of the picker's management pages: the add form and the remove-and-stop confirmation
//! inside the picker frame, the selected Machine's detail actions, the Add row, the action menu
//! anchored at its row and the inline rename.

use super::{component_state, picker_interaction, pointer_presentation, render_picker_close};
use crate::action_projection::ActionTone;
use crate::client_view::{Binding, KeyCode, LabelStyle, Modifiers, format_binding_with_glyphs};
use crate::localization::{Locale, ManageText};
use crate::machine_color::{MachineColor, Preset};
use crate::machine_picker::{
    AddForm, Check, Field, FormControl, MENU_KEY, MenuItem, Page, Picker, PickerAction,
    PickerSource, SWATCH_COLUMNS, Swatch,
};
use crate::machine_status::Action;
use crate::machines::added_label;
use crate::terminal_text::{display_width, line_editor::LineEditor, truncate_display};
use crate::tui_hit_map::{HitLayer, HitMapBuilder};
use crate::tui_render::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState,
    NavigationState, OperationState, PointerPresentation, RowContentBand, RowFocusCue, RowTracks,
    action_rect, paint_row_interaction, render_action,
};
use crate::tui_render::modal::{FieldTone, field_value_area, paint_field};
use crate::tui_theme::{Role, Theme};
use crate::workbench_state::field_window;
use ratatui::{
    Frame,
    layout::{Margin, Rect},
    style::{Modifier, Style},
    widgets::{Block, Clear, Padding, Paragraph},
};

/// The SSH config hosts the form lists at once; the list scrolls to the highlighted one.
const SUGGESTION_ROWS: usize = 4;

/// One action button of a page's action row.
pub(super) struct PageAction<'a> {
    pub(super) label: &'a str,
    pub(super) key: Binding,
    pub(super) kind: ActionKind,
    pub(super) destructive: bool,
    pub(super) action: PickerAction,
}

fn key_label(binding: &Binding, theme: &Theme) -> String {
    format_binding_with_glyphs(binding, LabelStyle::for_host(), theme.glyphs.key_labels())
}

/// Paints `actions` from the right end of `row` leftwards, each a picker target; returns the
/// leftmost painted column, or `row.right()` when none fits.
pub(super) fn render_page_actions(
    frame: &mut Frame<'_>,
    row: Rect,
    actions: &[PageAction<'_>],
    source: &PickerSource,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> u16 {
    let mut right = row.right();
    for action in actions.iter().rev() {
        let Some((target, spec)) = picker_interaction(source, action.action) else {
            continue;
        };
        let availability = if spec.enabled_for_pointer() {
            Availability::Enabled
        } else {
            Availability::Disabled
        };
        let key = key_label(&action.key, theme);
        let build = |pointer| {
            let spec = ActionSpec::new(
                action.label,
                action.kind,
                ControlSurface::Overlay,
                component_state(
                    pointer,
                    NavigationState::None,
                    CurrentState::NotCurrent,
                    CheckedState::Unchecked,
                    availability,
                    OperationState::Idle,
                ),
            )
            .with_shortcut(&key);
            if action.destructive {
                spec.with_tone(ActionTone::Destructive)
            } else {
                spec
            }
        };
        let available = Rect::new(row.x, row.y, right.saturating_sub(row.x), 1);
        let Some(measured) = action_rect(
            frame.area(),
            available,
            build(PointerPresentation::Rest),
            theme,
        ) else {
            break;
        };
        let rect = Rect::new(right - measured.width, row.y, measured.width, 1);
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &target,
            spec,
        ));
        let Some(painted) = render_action(frame, rect, build(pointer), theme) else {
            break;
        };
        hits.record_interaction(HitLayer::Overlay, painted, painted, target, spec);
        right = painted.x.saturating_sub(1);
    }
    right
}

/// Paints one single-line field with its caret when `focused`; a click on the row focuses it.
fn render_field(
    frame: &mut Frame<'_>,
    row: Rect,
    editor: &LineEditor,
    placeholder: Option<&str>,
    focused: bool,
    theme: &Theme,
) {
    let value_area = field_value_area(row, theme);
    let width = usize::from(value_area.width);
    let Some(window) = field_window(editor, width) else {
        return;
    };
    match placeholder {
        Some(placeholder) if editor.text().is_empty() => {
            let placeholder = truncate_display(placeholder, width, theme.glyphs.ellipsis);
            paint_field(frame, row, &placeholder, FieldTone::Placeholder, theme);
        }
        _ => {
            let tone = if window.selected() && focused {
                FieldTone::Selected
            } else {
                FieldTone::Value
            };
            paint_field(frame, row, window.display(), tone, theme);
        }
    }
    if focused {
        let column = u16::try_from(window.cursor_column())
            .unwrap_or(u16::MAX)
            .min(value_area.width.saturating_sub(1));
        frame.set_cursor_position((value_area.x.saturating_add(column), value_area.y));
    }
}

/// The color before the address, `●▾`: the marker in the machine's color and the disclosure of
/// its list. A click opens or closes the list, which keeps the marker pressed while it is open;
/// with focus, the caret rests on the marker. Returns the painted extent.
fn render_color_marker(
    frame: &mut Frame<'_>,
    row: Rect,
    form: &AddForm,
    source: &PickerSource,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Rect {
    let editable = form.editable();
    let marker = theme.glyphs.choice_on;
    let disclosure = theme.glyphs.disclosure_open;
    let marker_width = u16::try_from(display_width(marker)).unwrap_or(u16::MAX);
    let width = marker_width
        .saturating_add(u16::try_from(display_width(disclosure)).unwrap_or(u16::MAX))
        .min(row.width);
    let rect = Rect::new(row.x, row.y, width, 1);
    let interaction = picker_interaction(source, PickerAction::Form(FormControl::Palette));
    let pointer = interaction
        .as_ref()
        .map_or(PointerPresentation::Rest, |(target, spec)| {
            pointer_presentation(hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                target,
                *spec,
            ))
        });
    let surface = if form.palette.is_some() {
        Some(Role::SurfacePressed)
    } else {
        match pointer {
            PointerPresentation::Rest => None,
            PointerPresentation::Hovered => Some(Role::SurfaceHover),
            PointerPresentation::Pressed => Some(Role::SurfacePressed),
        }
    }
    .filter(|_| editable);
    let style = |role| {
        let style = theme.style(role);
        surface.map_or(style, |surface| style.patch(theme.style(surface)))
    };
    let (marker_style, disclosure_role) = if editable {
        (
            theme.machine_name_style(form.color, None, style(Role::TextPrimary)),
            Role::TextSecondary,
        )
    } else {
        (style(Role::TextDisabled), Role::TextDisabled)
    };
    frame.render_widget(
        Paragraph::new(marker).style(marker_style),
        Rect::new(rect.x, rect.y, marker_width.min(rect.width), 1),
    );
    frame.render_widget(
        Paragraph::new(disclosure).style(style(disclosure_role)),
        Rect::new(
            rect.x.saturating_add(marker_width),
            rect.y,
            rect.width.saturating_sub(marker_width),
            1,
        ),
    );
    if editable && form.field == Field::Color && form.palette.is_none() {
        frame.set_cursor_position((rect.x, rect.y));
    }
    if let Some((target, spec)) = interaction {
        hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
    }
    rect
}

/// The open color list below the color, as a select's list over the form: the presets in rows of
/// [`SWATCH_COLUMNS`], each marker and name in its color, then Custom…, which opens a field for a
/// color of the person's own on its line. The chosen entry is checked and the highlighted one
/// marked. A click on a preset chooses it; a click anywhere else closes the list.
#[expect(clippy::too_many_arguments, reason = "one painter of one list's parts")]
fn render_palette(
    frame: &mut Frame<'_>,
    body: Rect,
    anchor: Rect,
    form: &AddForm,
    highlighted: Swatch,
    source: &PickerSource,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    hits.start_modal();
    let bounds = frame.area();
    let heading = locale.machine_manage_text(ManageText::Color);
    let custom_label = locale.machine_manage_text(ManageText::CustomColor);
    let invalid = form
        .custom_invalid
        .then(|| locale.machine_manage_text(ManageText::CustomColorInvalid));
    let glyph_width = |glyph: &str| u16::try_from(display_width(glyph)).unwrap_or(u16::MAX);
    let navigation_width = glyph_width(theme.glyphs.navigation_marker).max(1);
    let check_width = glyph_width(theme.glyphs.checked_marker);
    let tracks = navigation_width + check_width;
    let marker_width = glyph_width(theme.glyphs.choice_on);
    let widest = Preset::ALL
        .iter()
        .map(|preset| glyph_width(locale.machine_color_name(MachineColor::Preset(*preset))))
        .max()
        .unwrap_or(0);
    // A cell: the two tracks, the marker between two gaps, the name; one blank cell between cells.
    let cell = tracks + 1 + marker_width + 1 + widest;
    let columns = u16::try_from(SWATCH_COLUMNS).unwrap_or(u16::MAX);
    let grid = cell * columns + columns.saturating_sub(1);
    // Custom…'s line: its cell, then the typed color's field of `#` and six digits with its padding.
    let custom_line = tracks + 1 + marker_width + 1 + glyph_width(custom_label) + 1 + 9;
    let content = grid
        .max(custom_line)
        .max(invalid.map_or(0, glyph_width))
        .max(glyph_width(heading) + 2);
    let width = content
        .saturating_add(4)
        .min(bounds.width.saturating_sub(2));
    let rows = Preset::ALL.len().div_ceil(SWATCH_COLUMNS);
    let height = u16::try_from(rows + 3 + usize::from(invalid.is_some())).unwrap_or(u16::MAX);
    if height > bounds.height {
        return;
    }
    let x = anchor.x.min(bounds.right().saturating_sub(width));
    let y = if anchor.bottom().saturating_add(height) <= bounds.bottom() {
        anchor.bottom()
    } else {
        anchor.y.saturating_sub(height)
    };
    let area = Rect::new(x, y, width, height);
    // The form's lines under the list are blanked across its width, so no cut-off text shows
    // beside the list.
    let covered = Rect::new(body.x, area.y, body.width, area.height).intersection(body);
    frame.render_widget(Clear, covered);
    frame.render_widget(
        Block::new().style(theme.style(Role::SurfaceOverlay)),
        covered,
    );
    frame.render_widget(Clear, area);
    let surface = crate::tui_render::overlay_surface(theme, Padding::horizontal(1));
    let inner = surface.inner(area);
    frame.render_widget(
        crate::tui_render::OverlayFrame::new(surface).heading(crate::tui_render::overlay_heading(
            heading,
            usize::from(width).saturating_sub(6),
            theme,
        )),
        area,
    );
    let painter = SwatchPainter {
        form,
        highlighted,
        source,
        theme,
        navigation_width,
        check_width,
        marker_width,
    };
    for (index, preset) in Preset::ALL.into_iter().enumerate() {
        let (row, column) = (index / SWATCH_COLUMNS, index % SWATCH_COLUMNS);
        let rect = Rect::new(
            inner.x + u16::try_from(column).unwrap_or(u16::MAX) * (cell + 1),
            inner.y + u16::try_from(row).unwrap_or(u16::MAX),
            cell,
            1,
        )
        .intersection(inner);
        painter.paint(
            frame,
            rect,
            Swatch::Preset(preset),
            locale.machine_color_name(MachineColor::Preset(preset)),
            hits,
        );
    }
    let custom_y = inner.y + u16::try_from(rows).unwrap_or(u16::MAX);
    let custom_cell = tracks + 1 + marker_width + 1 + glyph_width(custom_label);
    let custom_rect = Rect::new(inner.x, custom_y, custom_cell, 1).intersection(inner);
    painter.paint(frame, custom_rect, Swatch::Custom, custom_label, hits);
    if let Some(editor) = &form.custom {
        let field = Rect::new(
            custom_rect.right() + 1,
            custom_y,
            inner.right().saturating_sub(custom_rect.right() + 1).min(9),
            1,
        );
        render_field(
            frame,
            field,
            editor,
            Some(locale.machine_manage_text(ManageText::CustomColorHint)),
            true,
            theme,
        );
    }
    if let Some(invalid) = invalid {
        frame.render_widget(
            Paragraph::new(truncate_display(
                invalid,
                usize::from(inner.width),
                theme.glyphs.ellipsis,
            ))
            .style(theme.style(Role::StatusError)),
            Rect::new(inner.x, custom_y + 1, inner.width, 1).intersection(inner),
        );
    }
    if let Some((target, _)) = picker_interaction(source, PickerAction::Form(FormControl::Palette))
    {
        hits.record_overlay_dismissal(&target);
    }
}

/// What every entry of the open color list paints with.
struct SwatchPainter<'a> {
    form: &'a AddForm,
    highlighted: Swatch,
    source: &'a PickerSource,
    theme: &'a Theme,
    navigation_width: u16,
    check_width: u16,
    marker_width: u16,
}

impl SwatchPainter<'_> {
    /// One entry in `rect`: its tracks, its marker and its name, both in its color, raised on the
    /// highlight; Custom… takes the custom color's, or a hollow marker in the text color.
    fn paint(
        &self,
        frame: &mut Frame<'_>,
        rect: Rect,
        swatch: Swatch,
        name: &str,
        hits: &mut HitMapBuilder,
    ) {
        let theme = self.theme;
        if rect.width == 0 {
            return;
        }
        let Some((target, spec)) =
            picker_interaction(self.source, PickerAction::Form(FormControl::Swatch(swatch)))
        else {
            return;
        };
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &target,
            spec,
        ));
        let navigation = Rect::new(rect.x, rect.y, self.navigation_width, 1);
        let checked = Rect::new(navigation.right(), rect.y, self.check_width, 1);
        let dot = Rect::new(checked.right() + 1, rect.y, self.marker_width, 1);
        let primary = Rect::new(
            dot.right() + 1,
            rect.y,
            rect.right().saturating_sub(dot.right() + 1),
            1,
        );
        let name = truncate_display(name, usize::from(primary.width), theme.glyphs.ellipsis);
        let name_width = u16::try_from(display_width(&name))
            .unwrap_or(u16::MAX)
            .min(primary.width);
        frame.render_widget(
            Paragraph::new(name).style(theme.style(Role::TextPrimary)),
            primary,
        );
        let provisional = self.highlighted == swatch;
        let state = component_state(
            pointer,
            if provisional {
                NavigationState::Provisional
            } else {
                NavigationState::None
            },
            CurrentState::NotCurrent,
            if Swatch::of(self.form.color) == swatch {
                CheckedState::Checked
            } else {
                CheckedState::Unchecked
            },
            Availability::Enabled,
            OperationState::Idle,
        );
        let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
            .with_primary_role(Role::TextPrimary)
            .with_provisional(navigation)
            .with_checked(checked, theme.glyphs.checked_marker);
        let bands = [RowContentBand::explicit(dot)];
        if paint_row_interaction(
            frame,
            rect,
            tracks,
            &bands,
            ControlSurface::Overlay,
            state,
            theme,
        )
        .is_none()
        {
            return;
        }
        // Over the row's fill, the marker and a preset's name take the color.
        let color = match swatch {
            Swatch::Preset(preset) => Some(MachineColor::Preset(preset)),
            Swatch::Custom => {
                Some(self.form.color).filter(|color| matches!(color, MachineColor::Custom(_)))
            }
        };
        let raised = provisional.then_some(Role::SurfaceSelection);
        let (marker, marker_style) = match color {
            Some(color) => (
                theme.glyphs.choice_on,
                theme.machine_name_style(color, raised, Style::new()),
            ),
            None => (theme.glyphs.choice_off, theme.style(Role::TextSecondary)),
        };
        frame.render_widget(Paragraph::new(marker).style(marker_style), dot);
        if let Swatch::Preset(preset) = swatch {
            frame.buffer_mut().set_style(
                Rect::new(primary.x, primary.y, name_width, 1),
                theme.machine_name_style(MachineColor::Preset(preset), raised, Style::new()),
            );
        }
        hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
    }
}

/// The lines the SSH config hosts take under the address: their label and the hosts listed at once.
fn hosts_lines(form: &AddForm) -> u16 {
    if form.suggestions.is_empty() {
        0
    } else {
        1 + u16::try_from(form.suggestions.len().min(SUGGESTION_ROWS)).unwrap_or(u16::MAX)
    }
}

fn text_line(
    frame: &mut Frame<'_>,
    area: Rect,
    y: &mut u16,
    text: &str,
    role: Role,
    theme: &Theme,
) {
    if *y >= area.bottom() {
        return;
    }
    frame.render_widget(
        Paragraph::new(truncate_display(
            text,
            usize::from(area.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(role)),
        Rect::new(area.x, *y, area.width, 1),
    );
    *y += 1;
}

impl Picker {
    /// Paints a page that replaces the list inside the picker frame; false on the list and the
    /// pages that paint over it.
    pub(super) fn render_manage_page(
        &self,
        frame: &mut Frame<'_>,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) -> bool {
        let (title, rows) = match self.manage().page() {
            Page::Add(form) => (
                locale.machine_manage_text(ManageText::AddTitle),
                // Five lines for a check's outcome, and room for the color list below the
                // address, a typed custom color's problem included, when no host is listed.
                13 + hosts_lines(form),
            ),
            Page::Confirm { .. } => (locale.machine_manage_text(ManageText::ConfirmTitle), 8),
            Page::List | Page::Menu { .. } | Page::Rename { .. } => return false,
        };
        let area = frame.area();
        let width = area.width.saturating_sub(4).min(72);
        let height = area.height.saturating_sub(3).min(rows);
        if width < 34 || height < 6 {
            let _ = render_picker_close(frame, area, source, theme, hits);
            return true;
        }
        let panel = Rect::new(
            area.x + (area.width - width) / 2,
            area.y + (area.height - height) / 2,
            width,
            height,
        );
        frame.render_widget(
            crate::tui_render::titled_overlay(theme, Padding::ZERO, title, panel.width),
            panel,
        );
        let _ = render_picker_close(frame, panel, source, theme, hits);
        let inner = panel.inner(Margin::new(2, 1));
        match self.manage().page() {
            Page::Add(form) => self.render_add(frame, inner, form, source, locale, theme, hits),
            Page::Confirm { machine } => {
                let label = self
                    .manage()
                    .record(machine)
                    .map_or(machine.as_str(), |record| record.label.as_str());
                let label = crate::machine_discovery::display_label(label);
                let mut y = inner.y;
                text_line(frame, inner, &mut y, &label, Role::TextPrimary, theme);
                frame
                    .buffer_mut()
                    .set_style(Rect::new(inner.x, inner.y, inner.width, 1), Modifier::BOLD);
                let ends = locale.machine_ends_terminals(self.machine_terminals(machine), &label);
                text_line(frame, inner, &mut y, &ends, Role::TextSecondary, theme);
                let stay = locale.machine_manage_text(ManageText::FilesStay);
                text_line(frame, inner, &mut y, stay, Role::TextSecondary, theme);
                if self.manage().stopping(machine) {
                    y += 1;
                    let stopping = locale.machine_stopping(&label);
                    text_line(frame, inner, &mut y, &stopping, Role::StatusPending, theme);
                }
                let row = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
                render_page_actions(
                    frame,
                    row,
                    &[
                        PageAction {
                            label: locale.machine_manage_text(ManageText::Cancel),
                            key: Binding::new(KeyCode::Esc, Modifiers::NONE),
                            kind: ActionKind::Secondary,
                            destructive: false,
                            action: PickerAction::Form(FormControl::Cancel),
                        },
                        PageAction {
                            label: locale.machine_manage_text(ManageText::RemoveAndStop),
                            key: Binding::new(KeyCode::Enter, Modifiers::NONE),
                            kind: ActionKind::Primary,
                            destructive: true,
                            action: PickerAction::Form(FormControl::Submit),
                        },
                    ],
                    source,
                    theme,
                    hits,
                );
            }
            Page::List | Page::Menu { .. } | Page::Rename { .. } => {}
        }
        true
    }

    #[expect(clippy::too_many_arguments, reason = "one painter of one page's parts")]
    fn render_add(
        &self,
        frame: &mut Frame<'_>,
        inner: Rect,
        form: &AddForm,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        if inner.height < 4 {
            return;
        }
        let editable = form.editable();
        let body = Rect::new(inner.x, inner.y, inner.width, inner.height - 1);
        let mut y = body.y;
        let address = locale.machine_manage_text(ManageText::Address);
        text_line(frame, body, &mut y, address, Role::TextSecondary, theme);
        // The color, then the address on the same row; the name is the host's.
        let mut marker = None;
        if y < body.bottom() {
            let row = Rect::new(body.x, y, body.width, 1);
            let painted = render_color_marker(frame, row, form, source, theme, hits);
            marker = Some(painted);
            let gap = painted.width.saturating_add(1);
            let field = Rect::new(
                row.x.saturating_add(gap),
                y,
                row.width.saturating_sub(gap),
                1,
            );
            render_field(
                frame,
                field,
                &form.address,
                Some("user@10.0.0.8"),
                editable
                    && form.field == Field::Address
                    && form.suggestion.is_none()
                    && form.palette.is_none(),
                theme,
            );
            if let Some((target, spec)) = picker_interaction(
                source,
                PickerAction::Form(FormControl::Field(Field::Address)),
            ) {
                hits.record_interaction(HitLayer::Overlay, field, field, target, spec);
            }
            y += 1;
        }
        if !form.suggestions.is_empty() {
            let hosts = locale.machine_manage_text(ManageText::Hosts);
            text_line(frame, body, &mut y, hosts, Role::TextSecondary, theme);
            let top = form
                .suggestion
                .map_or(0, |index| index.saturating_sub(SUGGESTION_ROWS - 1));
            for (index, host) in form
                .suggestions
                .iter()
                .enumerate()
                .skip(top)
                .take(SUGGESTION_ROWS)
            {
                if y >= body.bottom() {
                    break;
                }
                let rect = Rect::new(body.x, y, body.width, 1);
                y += 1;
                let Some((target, spec)) =
                    picker_interaction(source, PickerAction::Form(FormControl::Suggestion(index)))
                else {
                    continue;
                };
                let pointer = pointer_presentation(hits.projected_pointer(
                    HitLayer::Overlay,
                    rect,
                    rect,
                    &target,
                    spec,
                ));
                let primary = Rect::new(rect.x + 2, rect.y, rect.width.saturating_sub(2), 1);
                let role = if editable {
                    Role::TextPrimary
                } else {
                    Role::TextDisabled
                };
                frame.render_widget(
                    Paragraph::new(truncate_display(
                        &crate::machine_discovery::display_label(host),
                        usize::from(primary.width),
                        theme.glyphs.ellipsis,
                    ))
                    .style(theme.style(role)),
                    primary,
                );
                let state = component_state(
                    pointer,
                    if form.suggestion == Some(index) {
                        NavigationState::Provisional
                    } else {
                        NavigationState::None
                    },
                    CurrentState::NotCurrent,
                    CheckedState::Unchecked,
                    if editable {
                        Availability::Enabled
                    } else {
                        Availability::Disabled
                    },
                    OperationState::Idle,
                );
                let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
                    .with_primary_role(role)
                    .with_provisional(Rect::new(rect.x, rect.y, 1, 1));
                if paint_row_interaction(
                    frame,
                    rect,
                    tracks,
                    &[],
                    ControlSurface::Overlay,
                    state,
                    theme,
                )
                .is_some()
                {
                    hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
                }
            }
        }
        y += 1;
        let typed = form.address.text().trim();
        let label = added_label(typed).unwrap_or_else(|| typed.to_owned());
        if let Some(problem) = form.problem {
            let text = locale.machine_manage_problem(problem);
            text_line(frame, body, &mut y, text, Role::StatusWarning, theme);
        }
        let mut submit = locale.machine_manage_text(ManageText::Add);
        match &form.check {
            None => {}
            Some(Check::Starting) => {
                let text = locale.machine_checking(crate::machine_add::Stage::Ssh);
                text_line(frame, body, &mut y, &text, Role::StatusPending, theme);
            }
            Some(Check::Checking(stage)) => {
                let text = locale.machine_checking(*stage);
                text_line(frame, body, &mut y, &text, Role::StatusPending, theme);
            }
            Some(Check::Failed {
                stage,
                explanation,
                retry,
            }) => {
                let text = locale.machine_check_failed(*stage);
                text_line(frame, body, &mut y, &text, Role::StatusError, theme);
                for line in locale.machine_status_lines(explanation, &label) {
                    text_line(frame, body, &mut y, &line, Role::TextSecondary, theme);
                }
                if *retry {
                    submit =
                        locale.machine_status_action(explanation.action.unwrap_or(Action::Retry));
                }
            }
            Some(Check::Saving) => {
                let text = locale.machine_saving();
                text_line(frame, body, &mut y, text, Role::StatusPending, theme);
            }
        }
        let row = Rect::new(inner.x, inner.bottom() - 1, inner.width, 1);
        render_page_actions(
            frame,
            row,
            &[
                PageAction {
                    label: locale.machine_manage_text(ManageText::Cancel),
                    key: Binding::new(KeyCode::Esc, Modifiers::NONE),
                    kind: ActionKind::Secondary,
                    destructive: false,
                    action: PickerAction::Form(FormControl::Cancel),
                },
                PageAction {
                    label: submit,
                    key: Binding::new(KeyCode::Enter, Modifiers::NONE),
                    kind: ActionKind::Primary,
                    destructive: false,
                    action: PickerAction::Form(FormControl::Submit),
                },
            ],
            source,
            theme,
            hits,
        );
        // The open list paints last, over the form, and alone takes clicks.
        if let (Some(highlighted), Some(marker)) = (form.palette, marker)
            && editable
        {
            render_palette(
                frame,
                body,
                marker,
                form,
                highlighted,
                source,
                locale,
                theme,
                hits,
            );
        }
    }

    /// The selected Machine's detail actions: its one helpful action on Enter, and the menu.
    pub(super) fn render_row_actions(
        &self,
        frame: &mut Frame<'_>,
        row: Rect,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) -> u16 {
        let index = self.selected();
        if self.manage().add_focused() || self.row_machine(index).is_none() {
            return row.right();
        }
        let mut actions = Vec::new();
        if let Some(action) = self.row_explanation(index).and_then(|e| e.action) {
            actions.push(PageAction {
                label: locale.machine_status_action(action),
                key: Binding::new(KeyCode::Enter, Modifiers::NONE),
                kind: ActionKind::Primary,
                destructive: false,
                action: PickerAction::RowAction(index),
            });
        }
        actions.push(PageAction {
            label: locale.machine_manage_text(ManageText::Actions),
            key: MENU_KEY,
            kind: ActionKind::Secondary,
            destructive: false,
            action: PickerAction::Menu(index),
        });
        render_page_actions(frame, row, &actions, source, theme, hits)
    }

    /// The fixed "Add machine…" row below the Machines list.
    pub(super) fn render_add_row(
        &self,
        frame: &mut Frame<'_>,
        rect: Rect,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let Some((target, spec)) = picker_interaction(source, PickerAction::AddMachine) else {
            return;
        };
        let pointer = pointer_presentation(hits.projected_pointer(
            HitLayer::Overlay,
            rect,
            rect,
            &target,
            spec,
        ));
        let primary = Rect::new(rect.x + 2, rect.y, rect.width.saturating_sub(3), 1);
        frame.render_widget(
            Paragraph::new(locale.machine_manage_text(ManageText::AddMachine))
                .style(theme.style(Role::TextPrimary)),
            primary,
        );
        let state = component_state(
            pointer,
            if self.manage().add_focused() {
                NavigationState::Provisional
            } else {
                NavigationState::None
            },
            CurrentState::NotCurrent,
            CheckedState::Unchecked,
            Availability::Enabled,
            OperationState::Idle,
        );
        let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
            .with_primary_role(Role::TextPrimary)
            .with_provisional(Rect::new(rect.x + 1, rect.y, 1, 1));
        if paint_row_interaction(
            frame,
            rect,
            tracks,
            &[],
            ControlSurface::Overlay,
            state,
            theme,
        )
        .is_some()
        {
            hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
        }
    }

    /// Paints the page drawn over the list: the action menu at the selected row, or the rename
    /// field in place of its name. The list below stays visible but takes no pointer.
    pub(super) fn render_manage_overlay(
        &self,
        frame: &mut Frame<'_>,
        row: Option<Rect>,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let Some(row) = row else {
            return;
        };
        let dismiss = picker_interaction(source, PickerAction::Close);
        match self.manage().page() {
            Page::Rename { name, .. } => {
                hits.start_modal();
                let field = Rect::new(row.x + 1, row.y, row.width.saturating_sub(2), 1);
                frame.render_widget(Clear, field);
                render_field(frame, field, name, None, true, theme);
                if let Some((target, _)) = &dismiss {
                    hits.record_overlay_dismissal(target);
                }
            }
            Page::Menu { machine, selected } => {
                hits.start_modal();
                let items = self.menu_items(machine);
                self.render_menu(
                    frame, row, machine, &items, *selected, source, locale, theme, hits,
                );
                if let Some((target, _)) = &dismiss {
                    hits.record_overlay_dismissal(target);
                }
            }
            Page::List | Page::Add(_) | Page::Confirm { .. } => {}
        }
    }

    #[expect(clippy::too_many_arguments, reason = "one painter of one menu's parts")]
    fn render_menu(
        &self,
        frame: &mut Frame<'_>,
        row: Rect,
        machine: &crate::machines::ProfileId,
        items: &[(MenuItem, bool)],
        selected: usize,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let bounds = frame.area();
        let label = self
            .manage()
            .record(machine)
            .map_or(machine.as_str(), |record| record.label.as_str());
        let label = crate::machine_discovery::display_label(label);
        let reason = items
            .get(selected)
            .filter(|(_, available)| !available)
            .map(|_| locale.machine_cannot_stop(&label));
        let widest = items
            .iter()
            .map(|(item, _)| display_width(locale.machine_menu_item(*item)))
            .max()
            .unwrap_or(0)
            .max(display_width(&label) + 2)
            .max(
                reason
                    .as_deref()
                    .map_or(0, |reason| display_width(reason) + 2),
            );
        // Two border cells, two padding cells and the navigation marker with its gap.
        let width = u16::try_from(widest + 6)
            .unwrap_or(u16::MAX)
            .min(bounds.width.saturating_sub(2));
        let height = u16::try_from(items.len() + 2).unwrap_or(u16::MAX);
        if width < 12 || height > bounds.height {
            return;
        }
        let x = row
            .x
            .saturating_add(2)
            .min(bounds.right().saturating_sub(width));
        let y = if row.bottom().saturating_add(height) <= bounds.bottom() {
            row.bottom()
        } else {
            row.y.saturating_sub(height)
        };
        let area = Rect::new(x, y, width, height);
        frame.render_widget(Clear, area);
        let surface = crate::tui_render::overlay_surface(theme, Padding::horizontal(1));
        let inner = surface.inner(area);
        frame.render_widget(
            crate::tui_render::OverlayFrame::new(surface).heading(
                crate::tui_render::overlay_heading(
                    &label,
                    usize::from(width).saturating_sub(6),
                    theme,
                ),
            ),
            area,
        );
        for (index, (item, available)) in items.iter().enumerate() {
            let rect = Rect::new(inner.x, inner.y + index as u16, inner.width, 1);
            if rect.y >= inner.bottom() {
                break;
            }
            let Some((target, spec)) = picker_interaction(source, PickerAction::MenuItem(*item))
            else {
                continue;
            };
            let pointer = pointer_presentation(hits.projected_pointer(
                HitLayer::Overlay,
                rect,
                rect,
                &target,
                spec,
            ));
            let primary = Rect::new(rect.x + 2, rect.y, rect.width.saturating_sub(2), 1);
            let role = if *available {
                Role::TextPrimary
            } else {
                Role::TextDisabled
            };
            frame.render_widget(
                Paragraph::new(locale.machine_menu_item(*item)).style(theme.style(role)),
                primary,
            );
            let state = component_state(
                pointer,
                if index == selected {
                    NavigationState::Provisional
                } else {
                    NavigationState::None
                },
                CurrentState::NotCurrent,
                CheckedState::Unchecked,
                if *available {
                    Availability::Enabled
                } else {
                    Availability::Disabled
                },
                OperationState::Idle,
            );
            let tracks = RowTracks::new(RowFocusCue::EnclosingFrame, primary)
                .with_primary_role(role)
                .with_provisional(Rect::new(rect.x, rect.y, 1, 1));
            let bands: [RowContentBand; 0] = [];
            if paint_row_interaction(
                frame,
                rect,
                tracks,
                &bands,
                ControlSurface::Overlay,
                state,
                theme,
            )
            .is_some()
            {
                hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
            }
        }
        if let Some(reason) = reason {
            crate::tui_render::action_menu::paint_edge_reason(frame, area, &reason, theme);
        }
    }
}
