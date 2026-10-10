//! Draw the complete Worktree decision before arming its input token.
use super::tab_close::{modal_area, set_modal_cursor, wrapped_text_fits};
use super::{
    Clear, Frame, Line, Locale, ModalPresentation, Padding, Paragraph, Rect, Theme, Wrap,
    display_width, truncate_display,
};
use crate::terminal_session::{
    WorktreeMode, WorktreeNotice, WorktreeStage, WorktreeToken, WorktreeView,
};
use crate::terminal_text::sanitize_terminal_text;
use crate::tui_pointer::{HitLayer, HitMapBuilder, HitTarget, ModalControl};
use crate::tui_theme::Role;
use ratatui::text::Text;
use unicode_segmentation::UnicodeSegmentation;

pub(super) fn render(
    frame: &mut Frame<'_>,
    locale: Locale,
    view: WorktreeView<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    hits.start_modal();
    if view.stage() == WorktreeStage::Checkout {
        return render_checkouts(
            frame,
            locale,
            CheckoutPicker {
                token: view.token(),
                mode: view.mode(),
                source: view.source_path().unwrap_or(""),
                count: view.choice_count(),
                selected: view.selected(),
            },
            content,
            theme,
            hits,
            |index| {
                view.choice(index).map(|(label, path)| CheckoutChoice {
                    label,
                    path,
                    openable: view.choice_openable(index),
                })
            },
        );
    }
    let stage = view.stage();
    let budget = usize::from(content.width.saturating_sub(10)).min(72);
    let mut lines = vec![
        Line::from(locale.worktree_title(view.mode())),
        Line::from(""),
    ];
    if let Some(source) = view.source_path() {
        lines.push(Line::from(path_text(source, budget, theme.glyphs.ellipsis)));
    }
    let mut field = None;
    match stage {
        WorktreeStage::Branch | WorktreeStage::Base => {
            lines.push(Line::from(locale.worktree_field(WorktreeStage::Branch)));
            let branch = lines.len();
            lines.push(Line::from(truncate_display(
                view.branch(),
                budget,
                theme.glyphs.ellipsis,
            )));
            lines.push(Line::from(locale.worktree_field(WorktreeStage::Base)));
            let base = lines.len();
            lines.push(Line::from(truncate_display(
                view.base(),
                budget,
                theme.glyphs.ellipsis,
            )));
            field = Some(if stage == WorktreeStage::Base {
                base
            } else {
                branch
            });
        }
        _ => {}
    }
    if let Some(id) = view.operation_id() {
        lines.push(Line::from(format!("orchbee worktree status {id}")));
    }
    if let Some(workspace) = view.workspace() {
        lines.push(Line::from(format!(
            "{}: {workspace}",
            locale.worktree_workspace()
        )));
    }
    if view.notice() != WorktreeNotice::Preparing || stage == WorktreeStage::Preparing {
        lines.push(Line::from(
            locale.worktree_stage_notice(view.notice(), stage),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(locale.worktree_actions(stage)));
    lines.push(Line::from(locale.setup_cancel_button()));
    let area = modal_area(content, &Text::from(lines.clone()));
    let block = super::overlay_surface(theme, true, Padding::horizontal(2));
    let inner = block.inner(area);
    let window = field.and_then(|_| view.field_window(usize::from(inner.width)));
    if let (Some(index), Some(window)) = (field, window.as_ref()) {
        lines[index] = Line::from(window.display().to_owned());
    }
    let text = Text::from(lines);
    let complete = wrapped_text_fits(&text, inner);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
    if complete {
        frame.render_widget(
            Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
            inner,
        );
        if let (Some(index), Some(window)) = (field, window.as_ref()) {
            set_modal_cursor(frame, &text, inner, index, window.cursor_column());
        }
        record_cancel(hits, view.token(), inner, locale);
    } else {
        frame.render_widget(
            Paragraph::new(locale.setup_resize_hint()).wrap(Wrap { trim: false }),
            inner,
        );
    }
    (
        area,
        if complete {
            ModalPresentation::Presented(view.token())
        } else {
            ModalPresentation::NotPresented
        },
    )
}

struct CheckoutPicker<'a> {
    token: WorktreeToken,
    mode: WorktreeMode,
    source: &'a str,
    count: usize,
    selected: usize,
}
struct CheckoutChoice<'a> {
    label: String,
    path: &'a str,
    openable: bool,
}

fn render_checkouts<'a>(
    frame: &mut Frame<'_>,
    locale: Locale,
    picker: CheckoutPicker<'_>,
    content: Rect,
    theme: &Theme,
    hits: &mut HitMapBuilder,
    choice: impl Fn(usize) -> Option<CheckoutChoice<'a>>,
) -> (Rect, ModalPresentation<WorktreeToken>) {
    let count = picker.count;
    let area = crate::tui_layout::centered_overlay(
        content,
        84.min(content.width.saturating_sub(4)),
        u16::try_from(count.saturating_mul(2).saturating_add(7))
            .unwrap_or(u16::MAX)
            .min(content.height.saturating_sub(2)),
    );
    let block = super::overlay_surface(theme, true, Padding::horizontal(2));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    if inner.width < 20 || inner.height < 7 || count == 0 {
        hits.record(HitLayer::OverlaySurface, area, HitTarget::OverlaySurface);
        frame.render_widget(
            Paragraph::new(locale.setup_resize_hint()).wrap(Wrap { trim: false }),
            inner,
        );
        return (area, ModalPresentation::NotPresented);
    }
    hits.record(
        HitLayer::OverlaySurface,
        area,
        HitTarget::Worktree {
            token: picker.token,
            control: ModalControl::Surface,
        },
    );
    let capacity = usize::from((inner.height - 5) / 2);
    let start = picker
        .selected
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    let budget = usize::from(inner.width);
    let mut lines = vec![
        Line::from(truncate_display(
            locale.worktree_title(picker.mode),
            budget,
            theme.glyphs.ellipsis,
        )),
        Line::from(path_text(picker.source, budget, theme.glyphs.ellipsis)),
    ];
    for index in start..count.min(start + capacity) {
        let CheckoutChoice {
            openable,
            label,
            path,
        } = choice(index).unwrap_or_else(|| CheckoutChoice {
            label: String::new(),
            path: "",
            openable: false,
        });
        let label = if openable {
            label
        } else {
            format!("{} · {label}", locale.setup_choice_unavailable())
        };
        let role = if !openable {
            Role::TextSecondary
        } else if index == picker.selected {
            Role::SurfaceSelection
        } else {
            Role::TextPrimary
        };
        lines.push(
            Line::from(format!(
                "{} {}",
                if index == picker.selected { ">" } else { " " },
                truncate_display(
                    &sanitize_terminal_text(&label),
                    budget.saturating_sub(2),
                    theme.glyphs.ellipsis
                )
            ))
            .style(theme.style(role)),
        );
        lines.push(
            Line::from(path_text(path, budget, theme.glyphs.ellipsis)).style(theme.style(role)),
        );
        if openable {
            hits.record(
                HitLayer::Overlay,
                Rect::new(
                    inner.x,
                    inner.y + 2 + u16::try_from((index - start) * 2).unwrap_or(0),
                    inner.width,
                    2,
                ),
                HitTarget::Worktree {
                    token: picker.token,
                    control: ModalControl::Choice(index),
                },
            );
        }
    }
    // Fixed footer rows do not participate in checkout scrolling.
    while lines.len() < usize::from(inner.height - 3) {
        lines.push(Line::from(""));
    }
    lines.push(Line::from(format!("{}/{}", picker.selected + 1, count)));
    let hint = locale.worktree_actions(WorktreeStage::Checkout);
    lines.push(Line::from(if display_width(hint) <= budget {
        hint
    } else {
        locale.setup_picker_compact_actions()
    }));
    lines.push(Line::from(locale.setup_cancel_button()));
    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
    record_cancel(hits, picker.token, inner, locale);
    (area, ModalPresentation::Presented(picker.token))
}

fn record_cancel(hits: &mut HitMapBuilder, token: WorktreeToken, inner: Rect, locale: Locale) {
    let width = u16::try_from(display_width(locale.setup_cancel_button())).unwrap_or(0);
    if inner.height > 0 && width <= inner.width {
        hits.record(
            HitLayer::Overlay,
            Rect::new(inner.x, inner.bottom() - 1, width, 1),
            HitTarget::Worktree {
                token,
                control: ModalControl::Cancel,
            },
        );
    }
}
fn path_text(path: &str, budget: usize, marker: &str) -> String {
    let clean = sanitize_terminal_text(path);
    if display_width(&clean) <= budget {
        return clean.into_owned();
    }
    let available = budget.saturating_sub(display_width(marker));
    let prefix = truncate_display(&clean, available / 2, "");
    let mut suffix = Vec::new();
    let mut width = 0;
    for g in clean.graphemes(true).rev() {
        let next = width + display_width(g);
        if next > available - available / 2 {
            break;
        }
        suffix.push(g);
        width = next;
    }
    format!(
        "{prefix}{marker}{}",
        suffix.into_iter().rev().collect::<String>()
    )
}

#[cfg(test)]
mod tests;
