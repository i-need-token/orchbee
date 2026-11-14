//! The login overlay's remember row: the `[ ] Remember password` box under the field, or, when no
//! store can be used in this login, one secondary line in its place. The box's target is its
//! marker and its whole label, and it shows keyboard focus while Tab put it there.

use super::super::controls::{
    Availability, CheckedState, ControlSurface, CurrentState, FocusState, InteractionState,
    NavigationState, OperationState, PointerPresentation, RowContentBand, RowFocusCue, RowTracks,
    paint_row_interaction,
};
use super::super::modal::DialogRow;
use crate::localization::{Locale, RememberText};
use crate::machine_login::RememberBox;
use crate::machine_picker::{LoginControl, LoginOverlay};
use crate::terminal_text::display_width;
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, HitTarget, InteractionAvailability, ProjectedPointer,
};
use crate::tui_theme::{Role, Theme};
use ratatui::{Frame, layout::Rect};

/// The box's label for this overlay, if it offers one.
fn label(login: &LoginOverlay, locale: Locale) -> Option<&'static str> {
    match login.presented.remember {
        RememberBox::Offered {
            until_restart: false,
            ..
        } => Some(locale.machine_remember_text(RememberText::Remember)),
        RememberBox::Offered {
            until_restart: true,
            ..
        } => Some(locale.machine_remember_text(RememberText::RememberUntilRestart)),
        RememberBox::Absent | RememberBox::Unavailable => None,
    }
}

/// The dialog row the remember box or its reason takes; none for a question whose answer is never
/// remembered. The box's row holds its whole text, so the dialog is laid out wide enough for it.
pub(super) fn row(login: &LoginOverlay, locale: Locale) -> Option<DialogRow<'static>> {
    if let Some(label) = label(login, locale) {
        return Some(DialogRow::Item(
            format!("[ ] {label}").into(),
            Role::TextPrimary,
        ));
    }
    (login.presented.remember == RememberBox::Unavailable).then(|| {
        DialogRow::Item(
            locale
                .machine_remember_text(RememberText::Unavailable)
                .into(),
            Role::TextSecondary,
        )
    })
}

/// Paints the box over its row `area` as a boolean control; the marker's cell when it holds the
/// keyboard, where the cursor goes.
pub(super) fn paint(
    frame: &mut Frame<'_>,
    login: &LoginOverlay,
    area: Rect,
    locale: Locale,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<(u16, u16)> {
    let label = label(login, locale)?;
    let width = u16::try_from(display_width(label).checked_add(4)?).ok()?;
    if area.height == 0 || width > area.width {
        return None;
    }
    let rect = Rect::new(area.x, area.y, width, 1);
    let target = HitTarget::MachineLogin {
        token: login.presented.token,
        control: LoginControl::Remember,
    };
    let enabled = !login.presented.sending;
    let availability = if enabled {
        InteractionAvailability::Enabled
    } else {
        InteractionAvailability::Disabled
    };
    let pointer = match hits.projected_modal_pointer(HitLayer::Overlay, rect, &target, availability)
    {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    };
    let ticked = login.ticked().unwrap_or(false);
    let focused = login.remember_focused();
    frame.buffer_mut().set_string(
        rect.x,
        rect.y,
        format!("[ ] {label}"),
        theme.style(Role::TextPrimary),
    );
    let tracks = RowTracks::new(
        RowFocusCue::EnclosingFrame,
        Rect::new(rect.x + 4, rect.y, width - 4, 1),
    )
    .with_primary_role(Role::TextPrimary)
    .with_checked(Rect::new(rect.x + 1, rect.y, 1, 1), "x");
    let bands = [
        RowContentBand::semantic(Rect::new(rect.x, rect.y, 1, 1), Role::TextSecondary),
        RowContentBand::semantic(Rect::new(rect.x + 2, rect.y, 2, 1), Role::TextSecondary),
    ];
    let state = InteractionState::new(
        if focused {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        pointer,
        NavigationState::None,
        CurrentState::NotCurrent,
        if ticked {
            CheckedState::Checked
        } else {
            CheckedState::Unchecked
        },
        if enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        OperationState::Idle,
    );
    let painted = paint_row_interaction(
        frame,
        rect,
        tracks,
        &bands,
        ControlSurface::Overlay,
        state,
        theme,
    )?;
    if painted != rect {
        return None;
    }
    hits.record_modal_control(HitLayer::Overlay, rect, target, availability);
    focused.then_some((rect.x + 1, rect.y))
}
