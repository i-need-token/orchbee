//! Paints a captured rename draft as its modal dialog using sanitized text and exact draft
//! identity. It does not edit the draft or commit a rename against the Server revision.

use super::controls::ActionKind;
use super::modal::{
    Dialog, DialogClass, DialogFooter, DialogRow, DialogTargets, ModalAction, NAME_FIELD_WIDTH,
    dialog_layout, field_value_area, modal_key_label, paint_dialog, paint_field,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::terminal_text::{sanitize_terminal_text, truncate_display};
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::Theme;
use crate::workbench_client::RenameDraftView;
use crate::workbench_state::rename_draft::RenameDraftToken;
use ratatui::{Frame, layout::Rect};

/// The rename dialog. A Tab's dialog names the Tab on its target line above the field, which
/// holds the label; an automatic name shows as the field's placeholder. A Panel's dialog keeps
/// the field alone, with its restore-the-original help.
pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &RenameDraftView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<RenameDraftToken>) {
    let placeholder_visible = draft.automatic_label() && draft.text().is_empty();
    let error = draft.field_error();
    let mut rows = Vec::with_capacity(4);
    if !draft.is_panel() {
        rows.push(DialogRow::Target(draft.target_label(locale)));
        rows.push(DialogRow::Blank);
    }
    let field_row = rows.len();
    rows.push(DialogRow::Field {
        minimum: NAME_FIELD_WIDTH,
    });
    rows.push(match error.filter(|_| draft.touched()) {
        Some(error) => DialogRow::Error(locale.field_error(error).into()),
        None => DialogRow::Help(draft.is_panel().then(|| locale.panel_rename_help().into())),
    });
    let escape = modal_key_label(theme, &Binding::new(KeyCode::Esc, Modifiers::NONE));
    let enter = modal_key_label(theme, &Binding::new(KeyCode::Enter, Modifiers::NONE));
    let actions = [
        ModalAction::new(locale.rename_cancel_label(), ActionKind::Secondary, true)
            .with_shortcut(&escape),
        ModalAction::new(
            locale.rename_submit_label(),
            ActionKind::Primary,
            error.is_none(),
        )
        .with_shortcut(&enter),
    ];
    let dialog = Dialog {
        title: if draft.is_panel() {
            locale.panel_rename_title()
        } else {
            locale.rename_title()
        },
        class: DialogClass::Narrow,
        rows,
        footer: DialogFooter::Actions(&actions),
    };
    let Some(layout) = dialog_layout(frame.area(), &dialog, theme) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let Some(row) = layout.row(field_row) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    // The raw draft stays exact; the field shows its safe projection, with the raw cursor mapped
    // through the same projection.
    let value_area = field_value_area(row, theme);
    let Some(window) = draft.field_window(usize::from(value_area.width)) else {
        return (Rect::default(), ModalPresentation::NotPresented);
    };
    let target = |submit| HitTarget::Rename {
        token: draft.token(),
        submit,
    };
    paint_dialog(
        frame,
        &dialog,
        &layout,
        &DialogTargets {
            close: target(false),
            actions: &[target(false), target(true)],
            dismiss_outside: !draft.touched(),
            scroll: None,
        },
        theme,
        hits,
    );
    if placeholder_visible {
        let label = draft.target_label(locale);
        let label = truncate_display(
            &sanitize_terminal_text(&label),
            usize::from(value_area.width),
            theme.glyphs.ellipsis,
        );
        paint_field(frame, row, &label, true, theme);
    } else {
        paint_field(frame, row, window.display(), false, theme);
    }
    let column = u16::try_from(window.cursor_column())
        .unwrap_or(u16::MAX)
        .min(value_area.width.saturating_sub(1));
    frame.set_cursor_position((value_area.x.saturating_add(column), value_area.y));
    (layout.area, ModalPresentation::Presented(draft.token()))
}
