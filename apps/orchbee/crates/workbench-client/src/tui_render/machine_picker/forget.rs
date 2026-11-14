//! The picker's "Forget password" action for a selected remote Machine, and the
//! status line that reports how a forget ended.

use super::picker_interaction;
use crate::client_view::{LabelStyle, format_binding_with_glyphs};
use crate::localization::{Locale, RememberText};
use crate::machine_discovery::Forgotten;
use crate::machine_picker::{FORGET_KEY, Picker, PickerAction, PickerSource};
use crate::terminal_text::display_width;
use crate::tui_hit_map::{HitLayer, HitMapBuilder, ProjectedPointer};
use crate::tui_theme::{Role, Theme};
use ratatui::{Frame, layout::Rect, style::Modifier, widgets::Paragraph};

impl Picker {
    /// Paints the action at the right of the details pane's action row when the selected row can
    /// forget and the action fits.
    pub(super) fn render_forget_action(
        &self,
        frame: &mut Frame<'_>,
        inner: Rect,
        source: &PickerSource,
        locale: Locale,
        theme: &Theme,
        hits: &mut HitMapBuilder,
    ) {
        let selected = self.selected();
        if self.forget_target(selected).is_none() || inner.height < 3 {
            return;
        }
        let label = format!(
            "[{} {}]",
            format_binding_with_glyphs(
                &FORGET_KEY,
                LabelStyle::for_host(),
                theme.glyphs.key_labels()
            ),
            locale.machine_remember_text(RememberText::Forget),
        );
        let width = u16::try_from(display_width(&label)).unwrap_or(u16::MAX);
        if width > inner.width {
            return;
        }
        let rect = Rect::new(inner.right() - width, inner.bottom() - 3, width, 1);
        let Some((target, spec)) =
            picker_interaction(source, PickerAction::ForgetPassword(selected))
        else {
            return;
        };
        let hovered = hits.projected_pointer(HitLayer::Overlay, rect, rect, &target, spec)
            != ProjectedPointer::Rest;
        frame.render_widget(
            Paragraph::new(label).style(theme.style(Role::HintKey).add_modifier(if hovered {
                Modifier::REVERSED
            } else {
                Modifier::UNDERLINED
            })),
            rect,
        );
        hits.record_interaction(HitLayer::Overlay, rect, rect, target, spec);
    }

    /// How the last forget ended, for the status line.
    pub(super) fn forget_status(&self, locale: Locale) -> Option<(&'static str, Role)> {
        let (text, role) = match self.forget_notice? {
            Forgotten::Removed => (RememberText::Forgotten, Role::StatusSuccess),
            Forgotten::NothingSaved => (RememberText::NothingSaved, Role::TextSecondary),
            Forgotten::Failed => (RememberText::ForgetFailed, Role::StatusWarning),
        };
        Some((locale.machine_remember_text(text), role))
    }
}
