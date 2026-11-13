//! The shared dialog composition: width classes, full-viewport placement, the fixed row order,
//! small-window degradation, the right-aligned action row and the field band. Feature owners
//! supply localized rows and their typed targets; they keep every effect.

use super::{ModalAction, paint_modal_action, paint_modal_close, paint_modal_frame};
use crate::terminal_text::{
    display_width, sanitize_terminal_text, truncate_display, truncate_display_middle, wrap_text,
};
use crate::tui_hit_map::{HitLayer, HitMapBuilder, HitTarget};
use crate::tui_render::controls::{KeyHint, PointerPresentation, action_rect, frame_close_rect};
use crate::tui_theme::{Role, Theme};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span, Text},
    widgets::Paragraph,
};
use std::borrow::Cow;

/// The content-width range a dialog resolves its size within, excluding frame and padding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::tui_render) enum DialogClass {
    /// Confirmations, notices and single-field forms: 40–56 cells.
    Narrow,
    /// Multi-field forms and pickers: 48–72 cells.
    Medium,
}

impl DialogClass {
    const fn range(self) -> (u16, u16) {
        match self {
            Self::Narrow => (40, 56),
            Self::Medium => (48, 72),
        }
    }

    /// The minimum of a path field, which takes the class maximum.
    pub(in crate::tui_render) const fn path_width(self) -> u16 {
        self.range().1
    }
}

/// The minimum of a name field.
pub(in crate::tui_render) const NAME_FIELD_WIDTH: u16 = 24;
/// The wide class's maximum content width, which also bounds a command line.
const WIDE_MAXIMUM: u16 = 88;

/// One semantic row of the fixed dialog order. Rows a feature omits close up without a gap.
pub(in crate::tui_render) enum DialogRow<'a> {
    /// Spacing; the first thing removed under height pressure.
    Blank,
    /// The exact object in `text-primary`; a name truncates at the end.
    Target(Cow<'a, str>),
    /// A path target, which truncates in the middle.
    TargetPath(Cow<'a, str>),
    /// What the task acts within, in `text-secondary`; removed after help under height pressure.
    Context(Cow<'a, str>),
    /// One sentence of prose; it wraps to two lines and truncates to one under height pressure.
    Prose(Cow<'a, str>),
    /// A required preview row in `role`, truncated at the end.
    Item(Cow<'a, str>, Role),
    /// A CLI command that is the only recovery, as code text on its own line in `text-secondary`.
    /// It stays whole: it widens the dialog past its class, up to the wide maximum.
    Command(Cow<'a, str>),
    /// Text that must be read whole, such as the sentence a question asks or a host key
    /// fingerprint, in `role`: it wraps at any width, a run longer than a line breaking anywhere,
    /// and is never cut, dropped or shortened; a dialog that cannot hold it cannot be drawn.
    /// `widen` lets its width widen the dialog past its class, up to the wide maximum.
    Whole {
        text: Cow<'a, str>,
        role: Role,
        widen: bool,
    },
    /// A field label; `accent-focus` marks the focused field of a multi-field form.
    Label(Cow<'a, str>, bool),
    /// One field row. The owner paints its value with [`paint_field`] inside the resolved row.
    Field { minimum: u16 },
    /// A field's reserved help row; it keeps its place without text so an error never moves rows.
    Help(Option<Cow<'a, str>>),
    /// The short heading of a field's suggestion list in `text-secondary`, truncated at the end;
    /// it goes with the context line under height pressure.
    Heading(Cow<'a, str>),
    /// A field error in the help row: the `status-error` glyph and one sentence.
    Error(Cow<'a, str>),
    /// The pending, failure or recovery state of the whole task.
    Status(Cow<'a, str>, Role),
    /// A picker list of `len` rows; it scrolls below its natural height.
    List { len: usize },
}

/// The last content row: an action row, or a picker's hint row.
pub(in crate::tui_render) enum DialogFooter<'a> {
    /// Secondary actions first and the primary last; at most three.
    Actions(&'a [ModalAction<'a>]),
    Hints(&'a [KeyHint<'a>]),
}

pub(in crate::tui_render) struct Dialog<'a> {
    pub(in crate::tui_render) title: &'a str,
    pub(in crate::tui_render) class: DialogClass,
    pub(in crate::tui_render) rows: Vec<DialogRow<'a>>,
    pub(in crate::tui_render) footer: DialogFooter<'a>,
}

/// The typed hit targets one dialog publishes, in footer order.
pub(in crate::tui_render) struct DialogTargets<'a> {
    pub(in crate::tui_render) close: HitTarget,
    pub(in crate::tui_render) actions: &'a [HitTarget],
    /// A press outside closes the dialog only when nothing would be lost: a picker, a notice, or
    /// a form whose fields are unchanged. Confirmations, changed forms and pending steps stay.
    pub(in crate::tui_render) dismiss_outside: bool,
    /// The wheel target of the dialog's list, if it has one.
    pub(in crate::tui_render) scroll: Option<HitTarget>,
}

/// Complete geometry admitted before a dialog mutates the frame or the hit map.
pub(in crate::tui_render) struct DialogLayout {
    pub(in crate::tui_render) area: Rect,
    close: Rect,
    /// Per owner row: its rectangle and resolved lines, or `None` when removed under pressure.
    rows: Vec<Option<(Rect, Vec<Line<'static>>)>>,
    footer: Rect,
    actions: Vec<Rect>,
    keys: bool,
}

impl DialogLayout {
    /// The rectangle of the owner's row `index`, when it survived height pressure.
    pub(in crate::tui_render) fn row(&self, index: usize) -> Option<Rect> {
        self.rows
            .get(index)
            .and_then(|row| row.as_ref().map(|(rect, _)| *rect))
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Drop {
    Blank,
    Help,
    Context,
}

impl DialogRow<'_> {
    const fn drop_kind(&self) -> Option<Drop> {
        match self {
            Self::Blank => Some(Drop::Blank),
            Self::Help(_) => Some(Drop::Help),
            Self::Context(_) | Self::Heading(_) => Some(Drop::Context),
            _ => None,
        }
    }

    /// The width this row requires before clamping; prose, help and errors never widen.
    fn required_width(&self) -> usize {
        match self {
            Self::Target(text) | Self::Label(text, _) => {
                display_width(&sanitize_terminal_text(text))
            }
            Self::Field { minimum } => usize::from(*minimum),
            _ => 0,
        }
    }

    /// The row's lines at `width`; `short` truncates wrapping prose to one line.
    fn lines(&self, width: u16, short: bool, theme: &Theme) -> Vec<Line<'static>> {
        let cells = usize::from(width);
        let ellipsis = theme.glyphs.ellipsis;
        let plain = |text: String, role: Role| vec![Line::styled(text, theme.style(role))];
        match self {
            Self::Blank | Self::Field { .. } | Self::Help(None) | Self::List { .. } => {
                vec![Line::from("")]
            }
            Self::Target(text) => plain(
                truncate_display(&sanitize_terminal_text(text), cells, ellipsis),
                Role::TextPrimary,
            ),
            Self::TargetPath(text) => plain(
                truncate_display_middle(&sanitize_terminal_text(text), cells, ellipsis),
                Role::TextPrimary,
            ),
            Self::Context(text) | Self::Heading(text) => plain(
                truncate_display(&sanitize_terminal_text(text), cells, ellipsis),
                Role::TextSecondary,
            ),
            Self::Item(text, role) => plain(
                truncate_display(&sanitize_terminal_text(text), cells, ellipsis),
                *role,
            ),
            Self::Command(text) => plain(
                truncate_display(&sanitize_terminal_text(text), cells, ellipsis),
                Role::TextSecondary,
            ),
            Self::Label(text, focused) => plain(
                truncate_display(&sanitize_terminal_text(text), cells, ellipsis),
                if *focused {
                    Role::AccentFocus
                } else {
                    Role::TextPrimary
                },
            ),
            Self::Whole { text, role, .. } => {
                let style = theme.style(*role);
                wrap_text(
                    &Text::from(sanitize_terminal_text(text).into_owned()),
                    width,
                )
                .lines
                .into_iter()
                .map(|line| {
                    Line::from(
                        line.spans
                            .into_iter()
                            .map(|span| Span::styled(span.content, style))
                            .collect::<Vec<_>>(),
                    )
                })
                .collect()
            }
            Self::Prose(text) => wrapped(text, None, Role::TextPrimary, width, short, theme),
            Self::Help(Some(text)) => wrapped(text, None, Role::TextSecondary, width, short, theme),
            Self::Error(text) => wrapped(
                text,
                Some(Role::StatusError),
                Role::TextPrimary,
                width,
                short,
                theme,
            ),
            Self::Status(text, role) => {
                wrapped(text, Some(*role), Role::TextPrimary, width, short, theme)
            }
        }
    }
}

/// The sentences of localized copy: each ends at `. `, `? `, `! `, `。`, `？` or `！`.
fn sentences(text: &str) -> Vec<&str> {
    let mut sentences = Vec::new();
    let mut start = 0;
    let mut characters = text.char_indices().peekable();
    while let Some((index, character)) = characters.next() {
        let end = index + character.len_utf8();
        let boundary = match character {
            '。' | '？' | '！' => true,
            '.' | '?' | '!' => characters.peek().is_some_and(|(_, next)| *next == ' '),
            _ => false,
        };
        if boundary && end < text.len() {
            sentences.push(text[start..end].trim());
            start = end;
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() || sentences.is_empty() {
        sentences.push(rest);
    }
    sentences
}

/// Starts each sentence on its own line and wraps it to at most two lines, or the whole text to
/// one line when `short`; a cut ends with `…`. A glyph row hangs its continuation under the text,
/// after the glyph and its blank.
fn wrapped(
    text: &str,
    glyph: Option<Role>,
    role: Role,
    width: u16,
    short: bool,
    theme: &Theme,
) -> Vec<Line<'static>> {
    let text = sanitize_terminal_text(text);
    let prefix = glyph.map_or(0, |_| display_width(theme.glyphs.status_bullet) + 1);
    let text_width = width.saturating_sub(u16::try_from(prefix).unwrap_or(u16::MAX));
    if text_width == 0 {
        return vec![Line::from("")];
    }
    let wrap = |sentence: &str, cap: usize| {
        let mut rows: Vec<String> = wrap_text(&Text::from(sentence), text_width)
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();
        if rows.len() > cap {
            // Wrapping proves the rest is wider than one row, so truncation always marks the cut.
            let rest = rows[cap - 1..].join(" ");
            rows.truncate(cap - 1);
            rows.push(truncate_display(
                &rest,
                usize::from(text_width),
                theme.glyphs.ellipsis,
            ));
        }
        rows
    };
    let rows = if short {
        wrap(&text, 1)
    } else {
        sentences(&text)
            .into_iter()
            .flat_map(|sentence| wrap(sentence, 2))
            .collect()
    };
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let mut spans = Vec::new();
            if let Some(glyph_role) = glyph {
                spans.push(if index == 0 {
                    Span::styled(
                        format!("{} ", theme.glyphs.status_bullet),
                        theme.style(glyph_role),
                    )
                } else {
                    Span::raw(" ".repeat(prefix))
                });
            }
            spans.push(Span::styled(row, theme.style(role)));
            Line::from(spans)
        })
        .collect()
}

/// The footer's width with or without key text inside the buttons.
fn footer_width(footer: &DialogFooter<'_>, keys: bool, theme: &Theme) -> Option<usize> {
    match footer {
        DialogFooter::Actions(actions) => {
            let mut width = 0usize;
            for (index, action) in actions.iter().enumerate() {
                let action = if keys { *action } else { action.bare() };
                let rect = action_rect(
                    Rect::new(0, 0, u16::MAX, 1),
                    Rect::new(0, 0, u16::MAX, 1),
                    action.spec(PointerPresentation::Rest),
                    theme,
                )?;
                width = width
                    .checked_add(usize::from(rect.width))?
                    .checked_add(usize::from(index > 0))?;
            }
            Some(width)
        }
        DialogFooter::Hints(hints) => {
            crate::tui_render::controls::measure_overlay_hints(hints, theme)
        }
    }
}

/// Resolves a dialog inside the whole viewport: its class width from the widest required line,
/// centered toward the top-left, then the small-window order of `layout.md`: the margin of the
/// axis that does not fit, then width below the class minimum and key text, then blank rows,
/// help, context, and prose beyond one line. `None` means the dialog cannot be drawn.
pub(in crate::tui_render) fn dialog_layout(
    viewport: Rect,
    dialog: &Dialog<'_>,
    theme: &Theme,
) -> Option<DialogLayout> {
    let (minimum, maximum) = dialog.class.range();
    let keyed = footer_width(&dialog.footer, true, theme)?;
    let required = dialog
        .rows
        .iter()
        .map(DialogRow::required_width)
        .chain([display_width(dialog.title).saturating_add(super::OVERLAY_TITLE_RESERVE - 6)])
        .max()
        .unwrap_or(0);
    // A command that is the only recovery is useless cut, so it may pass the class maximum; so
    // may whole text that reads best on one line, such as a fingerprint.
    let command = dialog
        .rows
        .iter()
        .filter_map(|row| match row {
            DialogRow::Command(text)
            | DialogRow::Whole {
                text, widen: true, ..
            } => Some(display_width(&sanitize_terminal_text(text))),
            _ => None,
        })
        .max()
        .map_or(0, |width| {
            u16::try_from(width).unwrap_or(u16::MAX).min(WIDE_MAXIMUM)
        });
    let preferred = u16::try_from(required)
        .unwrap_or(u16::MAX)
        .clamp(minimum, maximum)
        .max(command)
        .max(u16::try_from(keyed).ok()?);
    // Width: margin first, then narrow below the class while the complete action row fits,
    // then drop the buttons' key text.
    let mut keys = true;
    let width = if preferred.saturating_add(6) <= viewport.width {
        preferred
    } else {
        let available = viewport.width.checked_sub(6)?;
        if usize::from(available) < keyed {
            keys = false;
            if usize::from(available) < footer_width(&dialog.footer, false, theme)? {
                return None;
            }
        }
        available
    };
    let outer_width = width.checked_add(6)?;

    // Height: lists first shrink toward three rows inside the margin; then the margin; then
    // blank rows, help and context go in turn; then prose truncates and lists shrink to one row.
    let mut kept = vec![true; dialog.rows.len() + 2];
    let row = |index: usize| -> Option<&DialogRow<'_>> {
        index
            .checked_sub(1)
            .and_then(|index| dialog.rows.get(index))
    };
    let list_len = dialog
        .rows
        .iter()
        .find_map(|row| match row {
            DialogRow::List { len } => Some(*len),
            _ => None,
        })
        .unwrap_or(0);
    let height = |kept: &[bool], short: bool, list: usize| -> usize {
        let rows: usize = kept
            .iter()
            .enumerate()
            .filter(|(_, kept)| **kept)
            .map(|(index, _)| match row(index) {
                None => 1,
                Some(DialogRow::List { .. }) => list,
                Some(row) => row.lines(width, short, theme).len(),
            })
            .sum();
        rows + 3
    };
    let margin = usize::from(viewport.height.saturating_sub(2));
    let full = usize::from(viewport.height);
    let mut short = false;
    let fixed = height(&kept, false, 0);
    let mut list = list_len.min(margin.saturating_sub(fixed).max(list_len.min(3)));
    let mut limit = margin;
    if height(&kept, short, list) > limit {
        limit = full;
        list = list_len.min(full.saturating_sub(fixed).max(list_len.min(3)));
    }
    for drop in [Drop::Blank, Drop::Help, Drop::Context] {
        // Rows of one kind go one at a time from the bottom, so the top keeps its rhythm longest.
        for index in (0..kept.len()).rev() {
            if height(&kept, short, list) <= limit {
                break;
            }
            let kind = row(index).map_or(Some(Drop::Blank), DialogRow::drop_kind);
            if kind == Some(drop) {
                kept[index] = false;
            }
        }
    }
    if height(&kept, short, list) > limit {
        short = true;
    }
    if height(&kept, short, list) > limit && list_len > 0 {
        let rest = height(&kept, short, 0);
        list = limit.saturating_sub(rest).min(list_len);
    }
    let total = height(&kept, short, list);
    if total > limit || (list_len > 0 && list == 0) {
        return None;
    }
    let outer_height = u16::try_from(total).ok()?;
    let area = Rect::new(
        viewport.x + (viewport.width - outer_width) / 2,
        viewport.y + (viewport.height - outer_height) / 2,
        outer_width,
        outer_height,
    );
    let inner = Rect::new(area.x + 3, area.y + 1, width, outer_height - 2);
    let mut y = inner.y;
    let mut rows = Vec::with_capacity(dialog.rows.len());
    for (index, kept) in kept.iter().enumerate() {
        let entry = match row(index) {
            None => {
                y += u16::from(*kept);
                continue;
            }
            Some(_) if !*kept => None,
            Some(DialogRow::List { .. }) => {
                let rect = Rect::new(inner.x, y, width, u16::try_from(list).ok()?);
                y += rect.height;
                Some((rect, Vec::new()))
            }
            Some(row) => {
                let lines = row.lines(width, short, theme);
                let rect = Rect::new(inner.x, y, width, u16::try_from(lines.len()).ok()?);
                y += rect.height;
                Some((rect, lines))
            }
        };
        rows.push(entry);
    }
    let footer = Rect::new(inner.x, inner.bottom() - 1, width, 1);
    let mut actions = Vec::new();
    if let DialogFooter::Actions(specs) = &dialog.footer {
        let mut x =
            footer.right() - u16::try_from(footer_width(&dialog.footer, keys, theme)?).ok()?;
        for action in *specs {
            let action = if keys { *action } else { action.bare() };
            let rect = action_rect(
                viewport,
                Rect::new(x, footer.y, footer.right() - x, 1),
                action.spec(PointerPresentation::Rest),
                theme,
            )?;
            x = rect.right() + 1;
            actions.push(rect);
        }
    }
    let close = frame_close_rect(viewport, area)?;
    (viewport.intersection(area) == area).then_some(DialogLayout {
        area,
        close,
        rows,
        footer,
        actions,
        keys,
    })
}

/// Paints the frame, title, close control, every resolved row except fields and lists, and the
/// footer, publishing each control against its owner's exact target.
pub(in crate::tui_render) fn paint_dialog(
    frame: &mut Frame<'_>,
    dialog: &Dialog<'_>,
    layout: &DialogLayout,
    targets: &DialogTargets<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) {
    hits.start_modal();
    paint_modal_frame(frame, layout.area, dialog.title, theme);
    for (rect, lines) in layout.rows.iter().flatten() {
        if !lines.is_empty() {
            frame.render_widget(Paragraph::new(Text::from(lines.clone())), *rect);
        }
    }
    paint_modal_close(
        frame,
        layout.close,
        targets.close.clone(),
        targets.dismiss_outside,
        theme,
        hits,
    );
    match &dialog.footer {
        DialogFooter::Actions(actions) => {
            for ((rect, action), target) in layout.actions.iter().zip(*actions).zip(targets.actions)
            {
                let action = if layout.keys { *action } else { action.bare() };
                paint_modal_action(
                    frame,
                    Rect::new(rect.x, rect.y, layout.footer.right() - rect.x, 1),
                    *rect,
                    action,
                    target.clone(),
                    theme,
                    hits,
                );
            }
        }
        DialogFooter::Hints(hints) => {
            let _ = crate::tui_render::controls::render_overlay_hints(
                frame,
                layout.footer,
                hints,
                theme,
            );
        }
    }
    // Surface regions resolve in recording order: the list's wheel target precedes the frame's.
    if let Some(scroll) = &targets.scroll
        && let Some(list) = dialog
            .rows
            .iter()
            .position(|row| matches!(row, DialogRow::List { .. }))
            .and_then(|index| layout.row(index))
    {
        hits.record(HitLayer::OverlaySurface, list, scroll.clone());
    }
    hits.record(
        HitLayer::OverlaySurface,
        layout.area,
        HitTarget::OverlaySurface,
    );
}

/// The cells a field's value occupies inside its row: inside the band's padding, or inside the
/// bracketed boundary where the theme has no field fill.
pub(in crate::tui_render) fn field_value_area(row: Rect, theme: &Theme) -> Rect {
    let inset = if theme.field_band() { 1 } else { 2 };
    Rect::new(
        row.x.saturating_add(inset),
        row.y,
        row.width.saturating_sub(inset * 2),
        1,
    )
}

/// How a field row shows its text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::tui_render) enum FieldTone {
    /// The draft's own text in `text-primary`.
    Value,
    /// What an empty field stands for, in `text-secondary`.
    Placeholder,
    /// The draft's whole text, selected, on the `surface-selection` fill.
    Selected,
}

/// Paints one field row: a `surface-field` band with one padding cell at each end, or the
/// `[ value ]` fallback with `text-secondary` brackets. A placeholder uses `text-secondary`; a
/// selected value rests on `surface-selection` over its own cells only.
pub(in crate::tui_render) fn paint_field(
    frame: &mut Frame<'_>,
    row: Rect,
    value: &str,
    tone: FieldTone,
    theme: &Theme,
) {
    let value_area = field_value_area(row, theme);
    let role = if tone == FieldTone::Placeholder {
        Role::TextSecondary
    } else {
        Role::TextPrimary
    };
    let value = truncate_display(value, usize::from(value_area.width), "");
    let rest = " ".repeat(usize::from(value_area.width).saturating_sub(display_width(&value)));
    let (base, bracket) = if theme.field_band() {
        (Role::SurfaceField, None)
    } else {
        (Role::SurfaceOverlay, Some(theme.style(Role::TextSecondary)))
    };
    // Inside brackets the text keeps the dialog's own surface.
    let resting = if bracket.is_some() {
        theme.style(role)
    } else {
        theme.content_on_surface(base, None, Some(role), theme.style(role))
    };
    let value_style = if tone == FieldTone::Selected {
        theme.content_on_surface(
            base,
            Some(Role::SurfaceSelection),
            Some(role),
            theme.style(role),
        )
    } else {
        resting
    };
    let (open, close, edge) = match bracket {
        Some(bracket) => ("[ ", " ]", bracket),
        None => (" ", " ", theme.style(Role::SurfaceField)),
    };
    let line = Line::from(vec![
        Span::styled(open, edge),
        Span::styled(value, value_style),
        Span::styled(rest, resting),
        Span::styled(close, edge),
    ]);
    frame.render_widget(Paragraph::new(line), row);
}
