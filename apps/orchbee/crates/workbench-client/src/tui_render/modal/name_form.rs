//! The one single-field form every rename and group name shares: its title, the field, a reserved
//! help or error row and the Cancel and submit actions, with no line repeating the name above the
//! field. Feature owners supply the localized rows, typed targets and the draft's field window;
//! they keep every effect.

use super::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, FieldTone, ModalAction,
    dialog_layout, field_value_area, paint_dialog, paint_field,
};
use crate::terminal_text::truncate_display;
use crate::tui_hit_map::HitMapBuilder;
use crate::tui_theme::Theme;
use crate::workbench_state::FieldWindow;
use ratatui::{Frame, layout::Rect};
use std::borrow::Cow;

pub(in crate::tui_render) struct NameForm<'a> {
    pub(in crate::tui_render) title: &'a str,
    /// Rows naming what the task acts within, ahead of the field; a rename has none.
    pub(in crate::tui_render) lead: Vec<DialogRow<'a>>,
    pub(in crate::tui_render) field_minimum: u16,
    /// The field's help or error row, then any status of the whole task.
    pub(in crate::tui_render) trailing: Vec<DialogRow<'a>>,
    pub(in crate::tui_render) actions: &'a [ModalAction<'a>],
    /// The secondary text an empty field shows in place of a value.
    pub(in crate::tui_render) placeholder: Option<Cow<'a, str>>,
}

/// Paints `form` and its field from the draft's window, then places the caret; `None` when the
/// window cannot hold the dialog or its field.
pub(in crate::tui_render) fn paint_name_form<'w>(
    frame: &mut Frame<'_>,
    form: NameForm<'_>,
    window: impl FnOnce(usize) -> Option<FieldWindow<'w>>,
    targets: &DialogTargets<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    let field = form.lead.len();
    let mut rows = form.lead;
    rows.push(DialogRow::Field {
        minimum: form.field_minimum,
    });
    rows.extend(form.trailing);
    let dialog = Dialog {
        title: form.title,
        class: DialogClass::Narrow,
        rows,
        footer: DialogFooter::Actions(form.actions),
    };
    let layout = dialog_layout(frame.area(), &dialog, theme)?;
    let row = layout.row(field)?;
    // The raw draft stays exact; the field shows its safe projection, with the raw cursor mapped
    // through the same projection.
    let value_area = field_value_area(row, theme);
    let window = window(usize::from(value_area.width))?;
    paint_dialog(frame, &dialog, &layout, targets, theme, hits);
    match form.placeholder {
        Some(placeholder) => {
            let placeholder = truncate_display(
                &placeholder,
                usize::from(value_area.width),
                theme.glyphs.ellipsis,
            );
            paint_field(frame, row, &placeholder, FieldTone::Placeholder, theme);
        }
        None => {
            let tone = if window.selected() {
                FieldTone::Selected
            } else {
                FieldTone::Value
            };
            paint_field(frame, row, window.display(), tone, theme);
        }
    }
    let column = u16::try_from(window.cursor_column())
        .unwrap_or(u16::MAX)
        .min(value_area.width.saturating_sub(1));
    frame.set_cursor_position((value_area.x.saturating_add(column), value_area.y));
    Some(layout.area)
}
