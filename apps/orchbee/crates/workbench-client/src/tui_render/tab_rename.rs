//! Paints a captured rename draft as the shared name dialog using sanitized text and exact draft
//! identity. It does not edit the draft or commit a rename against the Server revision.

use super::controls::ActionKind;
use super::modal::{
    DialogRow, DialogTargets, ModalAction, NAME_FIELD_WIDTH, NameForm, modal_key_label,
    paint_name_form,
};
use crate::client_view::{Binding, KeyCode, Modifiers};
use crate::localization::Locale;
use crate::terminal_text::sanitize_terminal_text;
use crate::tui_hit_map::{HitMapBuilder, HitTarget};
use crate::tui_presentation::ModalPresentation;
use crate::tui_theme::Theme;
use crate::workbench_client::RenameDraftView;
use crate::workbench_state::rename_draft::RenameDraftToken;
use ratatui::{Frame, layout::Rect};

/// The rename dialog for a Tab or a Panel: the field holds the name, wholly selected when it is
/// prefilled, and an automatic name shows as the field's placeholder. A Panel's dialog keeps its
/// restore-the-original help.
pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    draft: &RenameDraftView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<RenameDraftToken>) {
    let error = draft.field_error();
    let help = match error.filter(|_| draft.touched()) {
        Some(error) => DialogRow::Error(locale.field_error(error).into()),
        None => DialogRow::Help(draft.is_panel().then(|| locale.panel_rename_help().into())),
    };
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
    let target = |submit| HitTarget::Rename {
        token: draft.token(),
        submit,
    };
    let form = NameForm {
        title: if draft.is_panel() {
            locale.panel_rename_title()
        } else {
            locale.rename_title()
        },
        lead: Vec::new(),
        field_minimum: NAME_FIELD_WIDTH,
        trailing: vec![help],
        actions: &actions,
        placeholder: (draft.automatic_label() && draft.text().is_empty()).then(|| {
            sanitize_terminal_text(&draft.target_label(locale))
                .into_owned()
                .into()
        }),
    };
    let targets = DialogTargets {
        close: target(false),
        actions: &[target(false), target(true)],
        dismiss_outside: !draft.touched(),
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
