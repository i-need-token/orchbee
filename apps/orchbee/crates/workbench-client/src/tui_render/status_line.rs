//! Typed projection and painting for the shell status: a floating chip, and the TooSmall row.

use crate::client_view::PrefixDesc;
use crate::client_view::{
    ClientAction, ClientView, KeyLabelGlyphs, LabelStyle, Notice, PanelDirection, PrefixAction,
    format_binding_with_glyphs, join_key_labels,
};
use crate::localization::PersistentStatus;
use crate::localization::{
    DisplayController, DisplayPhase, DisplayProvenance, Locale, TerminalEnd,
};
use crate::terminal_text::{display_width, sanitize_terminal_text, truncate_display};
use crate::tui_composition::CaptureOwner;
use crate::tui_layout::ViewportTier;
use crate::tui_theme::{Role, Theme};
use crate::workbench_client::WorkbenchClientView;
use crate::workbench_state::control_takeover::TakeoverStatus;
use crate::workbench_state::workspace_setup::{
    LocationLabelStatus, LocationLabelTarget, SetupStatus,
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

use super::controls::{KeyHint, measure_key_hints, render_key_hints};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputIndicator {
    Prefix,
    Navigator,
    Sidebar,
    Resize,
    ResizePending,
    /// The Find bar owns the keyboard.
    Find,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContextIndicator {
    PointerReleased,
    Zoomed,
    Panels(HiddenPanels),
}

impl ContextIndicator {
    fn label(self, locale: Locale) -> String {
        let copy = locale.status();
        match self {
            Self::PointerReleased => copy.pointer_released.to_owned(),
            Self::Zoomed => copy.zoomed.to_owned(),
            Self::Panels(panels) => locale.panel_position(panels.position, panels.total),
        }
    }
}

/// A split Tab showing only its focused Panel without being maximized: the tier draws no split,
/// or the split cannot keep its minimum pane size.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HiddenPanels {
    /// The shown Panel's one-based position in the Tab's canonical Panel order.
    pub(super) position: usize,
    pub(super) total: usize,
    /// A direction whose focus move reaches another Panel.
    pub(super) toward: Option<PanelDirection>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatusAction {
    ShowHelp,
    Exit,
    CreateTab,
    StartAgent,
    CreateWorkspace,
    Refresh,
    CheckAgain,
    RetryClose,
    TakeControl,
    RetryRename(LocationLabelTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StatusNotice<'a> {
    text: &'a str,
    marker: Role,
}

/// A marked notice as its sanitized forms, longest first; at most one is painted.
#[derive(Debug)]
struct NoticeForms {
    forms: Vec<String>,
    marker: Role,
}

#[derive(Debug, Eq, PartialEq)]
struct OwnedHint {
    key: String,
    label: &'static str,
}

#[derive(Debug)]
struct Indicator {
    label: &'static str,
    key: Option<String>,
    role: Role,
}

impl Indicator {
    fn width(&self) -> usize {
        display_width(self.label)
            + self
                .key
                .as_deref()
                .map_or(0, |key| 1usize.saturating_add(display_width(key)))
    }
}

/// Facts the shell selected for this frame; the status decides what fits.
#[derive(Clone, Copy, Debug)]
pub(super) struct StatusFacts<'a> {
    pub(super) primary: &'a str,
    /// The owner's documented shorter forms of `primary`, longest first. A partial fact is never
    /// drawn, so the first form that fits replaces a complete fact that does not.
    pub(super) primary_shorter: &'a [String],
    /// The ordinary Ready fact. The TooSmall row keeps it; the floating chip omits it, since the
    /// strip already names the current Tab.
    pub(super) primary_ordinary: bool,
    /// `primary` states a notice: when it leads and no form fits, its shortest form is cut at the
    /// end instead of dropped.
    pub(super) primary_notice: bool,
    /// A lasting state the chip names in a word or two beside the action that fixes it.
    pub(super) persistent: Option<PersistentStatus>,
    pub(super) secondary_notice: Option<&'a str>,
    /// The documented shorter forms of `secondary_notice`, longest first.
    pub(super) secondary_shorter: &'a [String],
    pub(super) zoomed: bool,
    pub(super) panels: Option<HiddenPanels>,
    pub(super) read_only: bool,
    pub(super) recovery_status: bool,
}

/// What one status paint covered.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct StatusPaint {
    /// The painted chip or row; its cells occlude the pane beneath.
    pub(super) surface: Option<Rect>,
    /// The complete primary fact, the recoverable-status draw receipt.
    pub(super) primary: Option<Rect>,
}

/// Paints the floating chip end-aligned in `area`, or the full TooSmall row.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    locale: Locale,
    facts: StatusFacts<'_>,
    tier: ViewportTier,
    theme: &Theme,
    session: Option<&WorkbenchClientView<'_>>,
) -> StatusPaint {
    if area.is_empty() {
        return StatusPaint::default();
    }
    if tier == ViewportTier::TooSmall {
        return render_row(
            frame,
            area,
            view,
            capture_owner,
            locale,
            facts,
            tier,
            theme,
            session,
        );
    }

    let client_notice = view
        .notice()
        .filter(|notice| !(facts.read_only && *notice == Notice::TerminalInputUnavailable));
    let primary = client_notice.map_or(facts.primary, |notice| locale.shell_notice(Some(notice)));
    let client_shorter = client_shorter(locale, client_notice);
    let secondary_notice = secondary_notice(view, primary, facts.secondary_notice);
    let glyphs = theme.glyphs.key_labels();
    let indicator_kind = input_indicator(view, capture_owner, facts.secondary_notice.is_some());
    let contexts = contexts(view, &facts);
    let copy = locale.status();
    let available = usize::from(area.width).saturating_sub(2);
    // An ended terminal that cannot name itself in two words still names its exit compactly.
    let persistent_text = facts.persistent.map(|persistent| {
        let (short, marker) = match persistent {
            PersistentStatus::DirectoryMissing => {
                (copy.directory_missing.to_owned(), Role::StatusWarning)
            }
            PersistentStatus::DirectoryUnavailable => {
                (locale.held_restore_title().to_owned(), Role::StatusWarning)
            }
            PersistentStatus::Disconnected => (copy.disconnected.to_owned(), Role::StatusError),
            PersistentStatus::TerminalEnded(TerminalEnd::Exited(_)) => {
                (copy.terminal_exited.to_owned(), Role::StatusWarning)
            }
            PersistentStatus::TerminalEnded(end @ TerminalEnd::Failed) => {
                (locale.terminal_end_compact(end), Role::StatusError)
            }
        };
        let fits = |text: &str| notice_width(StatusNotice { text, marker }, theme) <= available;
        let text = match persistent {
            PersistentStatus::TerminalEnded(end) if !fits(&short) => {
                Some(locale.terminal_end_compact(end)).filter(|compact| fits(compact))
            }
            _ => None,
        }
        .unwrap_or(short);
        (text, marker)
    });

    let (lead, hints) = if let Some(kind) = indicator_kind {
        (
            Lead::Indicator(indicator_projection(kind, view, locale, glyphs)),
            status_hints(
                view,
                capture_owner,
                Some(kind),
                client_notice,
                session,
                &contexts,
                tier,
                locale,
                glyphs,
            ),
        )
    } else if let Some((text, marker)) = persistent_text
        .as_ref()
        .filter(|_| client_notice.is_none() && secondary_notice.is_none())
    {
        (
            Lead::Notice(StatusNotice {
                text,
                marker: *marker,
            }),
            status_hints(
                view,
                capture_owner,
                None,
                client_notice,
                session,
                &[],
                tier,
                locale,
                glyphs,
            ),
        )
    } else {
        // A transient notice leads and the exact fact follows it, as the recoverable receipt
        // requires; the notice carries no hints of its own.
        let notice = secondary_notice.map(|text| NoticeForms {
            forms: sanitized_forms(text, facts.secondary_shorter),
            marker: Role::StatusWarning,
        });
        let primary_text = (client_notice.is_some() || !facts.primary_ordinary)
            .then_some(primary)
            .filter(|primary| !sanitize_terminal_text(primary).is_empty());
        // A Client notice replaces the fact, and with it the fact's shorter forms.
        let primary = primary_text.map_or_else(Vec::new, |primary| {
            sanitized_forms(
                primary,
                client_shorter.as_deref().unwrap_or(facts.primary_shorter),
            )
        });
        let hints = if notice.is_some() {
            HintSet::default()
        } else {
            status_hints(
                view,
                capture_owner,
                None,
                client_notice,
                session,
                &contexts,
                tier,
                locale,
                glyphs,
            )
        };
        let contexts = contexts
            .iter()
            .map(|context| context.label(locale))
            .collect();
        (
            Lead::Context {
                notice,
                primary,
                primary_notice: client_notice.is_some() || facts.primary_notice,
                contexts,
            },
            hints,
        )
    };
    let chip = layout_chip(
        lead,
        &hints,
        indicator_kind == Some(InputIndicator::Prefix),
        available,
        theme,
    );
    let painted = paint_chip(frame, area, &chip, theme);
    StatusPaint {
        surface: painted,
        primary: painted
            .zip(chip.primary)
            .filter(|_| client_notice.is_none())
            .map(|(rect, (offset, width))| {
                Rect::new(
                    rect.x
                        .saturating_add(1)
                        .saturating_add(u16::try_from(offset).unwrap_or(u16::MAX)),
                    rect.y,
                    u16::try_from(width).unwrap_or(u16::MAX),
                    1,
                )
            }),
    }
}

fn secondary_notice<'a>(
    view: &ClientView,
    primary: &str,
    secondary_notice: Option<&'a str>,
) -> Option<&'a str> {
    secondary_notice.filter(|notice| {
        !view.prefix_active()
            && !primary
                .strip_prefix(*notice)
                .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(" · "))
    })
}

fn contexts(view: &ClientView, facts: &StatusFacts<'_>) -> Vec<ContextIndicator> {
    let mut contexts = Vec::with_capacity(2);
    if view.pointer_released() {
        contexts.push(ContextIndicator::PointerReleased);
    }
    if facts.zoomed {
        contexts.push(ContextIndicator::Zoomed);
    } else if let Some(panels) = facts.panels {
        contexts.push(ContextIndicator::Panels(panels));
    }
    contexts
}

/// The chip's single lead: an input mode, a notice or lasting state, or context.
#[derive(Debug)]
enum Lead<'a> {
    Indicator(Indicator),
    Notice(StatusNotice<'a>),
    Context {
        notice: Option<NoticeForms>,
        /// The complete fact, then its shorter forms; at most one of them is painted.
        primary: Vec<String>,
        /// The primary states a notice, which is never dropped while it leads.
        primary_notice: bool,
        contexts: Vec<String>,
    },
}

/// The chip's laid-out content, inside its one-cell padding at each end.
#[derive(Debug, Default)]
struct ChipLayout {
    spans: Vec<Span<'static>>,
    width: usize,
    hints: Vec<OwnedHint>,
    hints_width: usize,
    /// The complete primary fact's offset and width inside the content.
    primary: Option<(usize, usize)>,
}

impl ChipLayout {
    fn push(&mut self, text: String, style: ratatui::style::Style) {
        self.width = self.width.saturating_add(display_width(&text));
        self.spans.push(Span::styled(text, style));
    }

    fn keep_hints(&mut self, (hints, width): (Vec<OwnedHint>, usize)) {
        self.hints = hints;
        self.hints_width = width;
    }

    fn is_empty(&self) -> bool {
        self.spans.is_empty() && self.hints.is_empty()
    }

    /// The separator a further lead part needs after what the lead already holds.
    fn joint_before(&self, separator: &Separator) -> usize {
        if self.spans.is_empty() {
            0
        } else {
            separator.width
        }
    }

    fn joint(&self, separator: &Separator) -> usize {
        if self.spans.is_empty() || self.hints.is_empty() {
            0
        } else {
            separator.width
        }
    }
}

/// Keeps the lead whole and drops hints whole, in their yield order, until the chip fits.
fn layout_chip(
    lead: Lead<'_>,
    hints: &HintSet,
    prefix: bool,
    available: usize,
    theme: &Theme,
) -> ChipLayout {
    let separator = separator(theme);
    let mut chip = ChipLayout::default();
    let room_after = |width: usize| {
        available
            .checked_sub(width)
            .and_then(|room| room.checked_sub(separator.width))
    };
    let fit_after = |width: usize| {
        room_after(width).map_or_else(|| (Vec::new(), 0), |room| fit_hints(hints, room, theme))
    };
    match lead {
        Lead::Indicator(mut indicator) => {
            if indicator.width() > available {
                indicator.key = None;
            }
            if indicator.width() > available {
                if !prefix {
                    chip.keep_hints(fit_hints(hints, available, theme));
                }
                return chip;
            }
            let kept = fit_after(indicator.width());
            if kept.0.is_empty() && !prefix && !hints.items.is_empty() {
                // The current return route outranks the mode's name.
                let alone = fit_hints(hints, available, theme);
                if !alone.0.is_empty() {
                    chip.keep_hints(alone);
                    return chip;
                }
            }
            let mut compact_key = None;
            if kept.0.is_empty()
                && prefix
                && let Some(hint) = hints.items.first()
            {
                // The prefix keeps its cancellation key, then the badge alone.
                let key = sanitize_terminal_text(&hint.key).into_owned();
                let key_width = display_width(&key);
                let fits = |indicator: &Indicator| {
                    room_after(indicator.width()).is_some_and(|room| key_width <= room)
                };
                if !fits(&indicator) && indicator.key.is_some() {
                    indicator.key = None;
                }
                if key_width > 0 && fits(&indicator) {
                    compact_key = Some(key);
                }
            }
            chip.push(indicator.label.to_owned(), theme.style(indicator.role));
            if let Some(key) = indicator.key {
                chip.push(" ".to_owned(), theme.style(Role::HintLabel));
                chip.push(key, theme.style(Role::HintKey));
            }
            if let Some(key) = compact_key {
                chip.push(separator.text.clone(), theme.style(Role::HintLabel));
                chip.push(key, theme.style(Role::HintKey));
            }
            chip.keep_hints(kept);
        }
        Lead::Notice(notice) => {
            if notice_width(notice, theme) > available {
                chip.keep_hints(fit_hints(hints, available, theme));
                return chip;
            }
            chip.push(
                format!("{} ", theme.glyphs.status_bullet),
                theme.style(notice.marker),
            );
            chip.push(
                sanitize_terminal_text(notice.text).into_owned(),
                theme.style(Role::TextPrimary),
            );
            chip.keep_hints(fit_after(chip.width));
        }
        Lead::Context {
            notice,
            primary,
            primary_notice,
            contexts,
        } => {
            if let Some(notice) = notice
                && let Some(text) = available
                    .checked_sub(notice_marker_width(theme))
                    .and_then(|room| notice_form(&notice.forms, room, theme))
            {
                chip.push(
                    format!("{} ", theme.glyphs.status_bullet),
                    theme.style(notice.marker),
                );
                chip.push(text, theme.style(Role::TextPrimary));
            }
            // The first form that fits leads; only the complete fact is the draw receipt.
            let leads = chip.spans.is_empty();
            let mut placed = false;
            for (form_index, form) in primary.iter().enumerate() {
                let joint = chip.joint_before(&separator);
                let width = display_width(form);
                if chip.width.saturating_add(joint).saturating_add(width) <= available {
                    if joint > 0 {
                        chip.push(separator.text.clone(), theme.style(Role::HintLabel));
                    }
                    if form_index == 0 {
                        chip.primary = Some((chip.width, width));
                    }
                    chip.push(form.clone(), theme.style(Role::TextPrimary));
                    placed = true;
                    break;
                }
            }
            if !placed
                && leads
                && primary_notice
                && let Some(text) = primary
                    .last()
                    .and_then(|form| cut_notice(form, available, theme))
            {
                chip.push(text, theme.style(Role::TextPrimary));
            }
            for context in contexts {
                let joint = chip.joint_before(&separator);
                if chip
                    .width
                    .saturating_add(joint)
                    .saturating_add(display_width(&context))
                    > available
                {
                    break;
                }
                if joint > 0 {
                    chip.push(separator.text.clone(), theme.style(Role::HintLabel));
                }
                chip.push(context, theme.style(Role::TextPrimary));
            }
            if chip.spans.is_empty() {
                chip.keep_hints(fit_hints(hints, available, theme));
            } else {
                chip.keep_hints(fit_after(chip.width));
            }
        }
    }
    chip
}

/// Fills the chip with the overlay surface, end-aligned so it covers only its own cells.
fn paint_chip(frame: &mut Frame<'_>, area: Rect, chip: &ChipLayout, theme: &Theme) -> Option<Rect> {
    if chip.is_empty() {
        return None;
    }
    let separator = separator(theme);
    let joint = chip.joint(&separator);
    let lead = u16::try_from(chip.width.saturating_add(joint)).ok()?;
    let content = lead.checked_add(u16::try_from(chip.hints_width).ok()?)?;
    let width = content
        .checked_add(2)
        .filter(|width| *width <= area.width)?;
    let rect = Rect::new(area.right() - width, area.y, width, 1);
    if frame.area().intersection(rect) != rect {
        return None;
    }
    frame.render_widget(Clear, rect);
    frame
        .buffer_mut()
        .set_style(rect, theme.style(Role::SurfaceOverlay));
    let x = rect.x + 1;
    if lead > 0 {
        let mut spans = chip.spans.clone();
        if joint > 0 {
            spans.push(Span::styled(
                separator.text.clone(),
                theme.style(Role::HintLabel),
            ));
        }
        frame.render_widget(
            Paragraph::new(Line::from(spans)),
            Rect::new(x, rect.y, lead, 1),
        );
    }
    if !chip.hints.is_empty() {
        let hints = borrowed_hints(&chip.hints);
        let width = u16::try_from(chip.hints_width).ok()?;
        let _ = render_key_hints(frame, Rect::new(x + lead, rect.y, width, 1), &hints, theme);
    }
    Some(rect)
}

/// One marked notice, with its documented shorter forms, and an optional hint as a floating chip.
/// With `hint_first`, the hint is the owner's return route and survives narrowing ahead of the
/// notice. The notice is never dropped for width while any of it fits: its first form that fits,
/// else its shortest form cut at the end.
#[allow(clippy::too_many_arguments)]
pub(super) fn render_notice_chip(
    frame: &mut Frame<'_>,
    area: Rect,
    notice: &str,
    shorter: &[String],
    marker: Role,
    hint: Option<(String, &'static str)>,
    hint_first: bool,
    theme: &Theme,
) -> Option<Rect> {
    let available = usize::from(area.width).saturating_sub(2);
    let separator = separator(theme);
    let forms = sanitized_forms(notice, shorter);
    let marker_width = notice_marker_width(theme);
    let hints = HintSet::from(
        hint.map(|(key, label)| OwnedHint { key, label })
            .into_iter()
            .collect::<Vec<_>>(),
    );
    let (alone, alone_width) = fit_hints(&hints, available, theme);
    let beside_hint = available
        .checked_sub(alone_width.saturating_add(separator.width))
        .and_then(|room| room.checked_sub(marker_width));
    let complete_beside_hint = forms
        .first()
        .zip(beside_hint)
        .is_some_and(|(complete, room)| display_width(complete) <= room);
    let alone_notice = || {
        available
            .checked_sub(marker_width)
            .and_then(|room| notice_form(&forms, room, theme))
    };
    let (text, show_hint) = if alone.is_empty() {
        (alone_notice(), false)
    } else if complete_beside_hint {
        (forms.first().cloned(), true)
    } else if hint_first {
        (
            beside_hint.and_then(|room| notice_form(&forms, room, theme)),
            true,
        )
    } else {
        alone_notice().map_or((None, true), |text| (Some(text), false))
    };
    let mut chip = ChipLayout::default();
    if let Some(text) = text {
        chip.push(
            format!("{} ", theme.glyphs.status_bullet),
            theme.style(marker),
        );
        chip.push(text, theme.style(Role::TextPrimary));
    }
    if show_hint {
        chip.keep_hints((alone, alone_width));
    }
    paint_chip(frame, area, &chip, theme)
}

/// Context and hints as a floating chip for a gesture's feedback. The hints are its return route
/// and survive narrowing ahead of the context.
pub(super) fn render_context_chip(
    frame: &mut Frame<'_>,
    area: Rect,
    contexts: Vec<&'static str>,
    hints: Vec<(String, &'static str)>,
    theme: &Theme,
) -> Option<Rect> {
    let available = usize::from(area.width).saturating_sub(2);
    let hints = HintSet::from(
        hints
            .into_iter()
            .map(|(key, label)| OwnedHint { key, label })
            .collect::<Vec<_>>(),
    );
    let separator = separator(theme);
    let context_width = contexts
        .iter()
        .map(|context| display_width(context))
        .sum::<usize>()
        .saturating_add(
            separator
                .width
                .saturating_mul(contexts.len().saturating_sub(1)),
        );
    let complete = measure_key_hints(&borrowed_hints(&hints.items), theme).is_some_and(|width| {
        context_width
            .saturating_add(separator.width)
            .saturating_add(width)
            <= available
    });
    let chip = if complete {
        layout_chip(
            Lead::Context {
                notice: None,
                primary: Vec::new(),
                primary_notice: false,
                contexts: contexts.into_iter().map(str::to_owned).collect(),
            },
            &hints,
            false,
            available,
            theme,
        )
    } else {
        // The context yields whole; the cancellation route stays.
        let mut chip = ChipLayout::default();
        chip.keep_hints(fit_hints(&hints, available, theme));
        chip
    };
    paint_chip(frame, area, &chip, theme)
}

/// The TooSmall row: the primary fact at the start and the exit hint at the end.
#[allow(clippy::too_many_arguments)]
fn render_row(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    locale: Locale,
    facts: StatusFacts<'_>,
    tier: ViewportTier,
    theme: &Theme,
    session: Option<&WorkbenchClientView<'_>>,
) -> StatusPaint {
    frame.render_widget(Clear, area);
    frame
        .buffer_mut()
        .set_style(area, theme.style(Role::SurfaceChrome));
    let client_notice = view
        .notice()
        .filter(|notice| !(facts.read_only && *notice == Notice::TerminalInputUnavailable));
    let primary = client_notice.map_or(facts.primary, |notice| locale.shell_notice(Some(notice)));
    let client_shorter = client_shorter(locale, client_notice);
    let primary_shorter = client_shorter.as_deref().unwrap_or(facts.primary_shorter);
    let primary_notice = client_notice.is_some() || facts.primary_notice;
    let secondary_notice = secondary_notice(view, primary, facts.secondary_notice);
    let glyphs = theme.glyphs.key_labels();
    let indicator_kind = input_indicator(view, capture_owner, facts.secondary_notice.is_some());
    let contexts = contexts(view, &facts);

    let notice = (indicator_kind.is_none())
        .then_some(secondary_notice)
        .flatten();
    // The notice is never dropped for width: its first form that fits, else its shortest cut.
    let notice_text = notice.and_then(|text| {
        usize::from(area.width)
            .checked_sub(notice_marker_width(theme))
            .and_then(|room| {
                notice_form(&sanitized_forms(text, facts.secondary_shorter), room, theme)
            })
    });
    let hints = if notice.is_some() {
        Vec::new()
    } else {
        status_hints(
            view,
            capture_owner,
            indicator_kind,
            client_notice,
            session,
            &contexts,
            tier,
            locale,
            glyphs,
        )
        .items
    };
    let mut indicator =
        indicator_kind.map(|value| indicator_projection(value, view, locale, glyphs));

    let separator = separator(theme);
    let primary = sanitize_terminal_text(primary).into_owned();
    let primary_width = display_width(&primary);
    let mut right_width = 0usize;
    let mut visible_hints = 0usize;
    let mut compact_hint_key = None;
    let mut hide_indicator = false;
    let mut right_is_notice = false;

    if let Some(indicator) = &mut indicator {
        if indicator.width() > usize::from(area.width) {
            indicator.key = None;
        }
        let indicator_width = indicator.width();
        if indicator_width <= usize::from(area.width) {
            right_width = indicator_width;
            let remaining = usize::from(area.width)
                .saturating_sub(indicator_width)
                .saturating_sub(separator.width);
            let (count, width) = fitting_hints(&hints, remaining, theme);
            if count > 0 {
                visible_hints = count;
                right_width = right_width
                    .saturating_add(separator.width)
                    .saturating_add(width);
            } else if indicator_kind == Some(InputIndicator::Prefix)
                && let Some(hint) = hints.first()
            {
                let key = sanitize_terminal_text(&hint.key).into_owned();
                let key_width = display_width(&key);
                let mut required = indicator_width
                    .saturating_add(separator.width)
                    .saturating_add(key_width);
                if required > usize::from(area.width) && indicator.key.is_some() {
                    indicator.key = None;
                    right_width = indicator.width();
                    required = right_width
                        .saturating_add(separator.width)
                        .saturating_add(key_width);
                }
                if key_width > 0 && required <= usize::from(area.width) {
                    compact_hint_key = Some(key);
                    right_width = required;
                }
            }
        }
        if visible_hints == 0 && indicator_kind != Some(InputIndicator::Prefix) && !hints.is_empty()
        {
            let (count, width) = fitting_hints(&hints, usize::from(area.width), theme);
            if count > 0 {
                visible_hints = count;
                right_width = width;
                hide_indicator = true;
            }
        }
    } else if let Some(text) = &notice_text {
        right_width = notice_width(
            StatusNotice {
                text,
                marker: Role::StatusWarning,
            },
            theme,
        );
        right_is_notice = true;
    } else if !hints.is_empty() {
        let (count, width) = fitting_hints(&hints, usize::from(area.width), theme);
        visible_hints = count;
        right_width = width;
    }

    if hide_indicator {
        indicator = None;
    }

    if facts.recovery_status
        && indicator.is_none()
        && notice.is_none()
        && primary_width <= usize::from(area.width)
    {
        let required = primary_width
            .saturating_add(if right_width > 0 { separator.width } else { 0 })
            .saturating_add(right_width);
        if required > usize::from(area.width) {
            right_width = 0;
            visible_hints = 0;
            right_is_notice = false;
        }
    }

    let right_width_u16 = u16::try_from(right_width)
        .unwrap_or(u16::MAX)
        .min(area.width);
    let left_gap = u16::from(right_width_u16 > 0);
    let left = Rect::new(
        area.x,
        area.y,
        area.width
            .saturating_sub(right_width_u16)
            .saturating_sub(left_gap),
        1,
    );
    let primary_visible = super::modal::unwrapped_text_fits(&primary, left);
    let primary_rect = primary_visible.then(|| {
        Rect::new(
            left.x,
            left.y,
            u16::try_from(primary_width).unwrap_or(u16::MAX),
            1,
        )
    });
    // A partial fact is never drawn: the first shorter form that fits replaces it, without
    // becoming the complete fact's draw receipt. A notice is never dropped for width: its
    // shortest form is cut instead.
    let shorter = if primary_visible {
        None
    } else {
        primary_shorter
            .iter()
            .map(|form| sanitize_terminal_text(form).into_owned())
            .find(|form| !form.is_empty() && super::modal::unwrapped_text_fits(form, left))
            .or_else(|| {
                primary_notice
                    .then(|| {
                        primary_shorter
                            .last()
                            .map_or(primary.as_str(), String::as_str)
                    })
                    .and_then(|form| {
                        cut_notice(
                            &sanitize_terminal_text(form),
                            usize::from(left.width),
                            theme,
                        )
                    })
            })
    };
    if primary_visible {
        render_primary(frame, left, &primary, &contexts, locale, theme);
    } else if let Some(form) = &shorter {
        render_primary(frame, left, form, &contexts, locale, theme);
    }

    if right_width_u16 > 0 {
        let right = Rect::new(area.right() - right_width_u16, area.y, right_width_u16, 1);
        if (primary_visible || shorter.is_some()) && left_gap > 0 {
            frame.buffer_mut().set_string(
                right.x - 1,
                right.y,
                " ",
                theme.style(Role::SurfaceChrome),
            );
        }
        if right_is_notice {
            if let Some(text) = &notice_text {
                render_notice(
                    frame,
                    right,
                    StatusNotice {
                        text,
                        marker: Role::StatusWarning,
                    },
                    theme,
                );
            }
        } else {
            render_indicator_and_hints(
                frame,
                right,
                indicator.as_ref(),
                &hints[..visible_hints],
                compact_hint_key.as_deref(),
                &separator,
                theme,
            );
        }
    }

    StatusPaint {
        surface: Some(area),
        primary: if client_notice.is_none() {
            primary_rect
        } else {
            None
        },
    }
}

#[derive(Debug)]
struct Separator {
    text: String,
    width: usize,
}

fn separator(theme: &Theme) -> Separator {
    let glyph = sanitize_terminal_text(theme.glyphs.middle_dot);
    let width = display_width(&glyph).saturating_add(2);
    Separator {
        text: format!(" {glyph} "),
        width,
    }
}

fn render_primary(
    frame: &mut Frame<'_>,
    area: Rect,
    primary: &str,
    contexts: &[ContextIndicator],
    locale: Locale,
    theme: &Theme,
) {
    let mut spans = vec![Span::styled(
        primary.to_owned(),
        theme.style(Role::TextPrimary),
    )];
    let mut used = display_width(primary);
    let separator = separator(theme);
    for context in contexts {
        let label = context.label(locale);
        let next = used
            .saturating_add(separator.width)
            .saturating_add(display_width(&label));
        if next > usize::from(area.width) {
            break;
        }
        spans.push(Span::styled(
            separator.text.clone(),
            theme.style(Role::HintLabel),
        ));
        spans.push(Span::styled(label, theme.style(Role::TextPrimary)));
        used = next;
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn notice_width(notice: StatusNotice<'_>, theme: &Theme) -> usize {
    notice_marker_width(theme).saturating_add(display_width(&sanitize_terminal_text(notice.text)))
}

/// The cells a notice's severity marker and the space after it take.
fn notice_marker_width(theme: &Theme) -> usize {
    display_width(theme.glyphs.status_bullet).saturating_add(1)
}

/// A Client notice's documented shorter form, which replaces the fact's own.
fn client_shorter(locale: Locale, notice: Option<Notice>) -> Option<Vec<String>> {
    notice.map(|notice| {
        locale
            .shell_notice_shorter(notice)
            .map(str::to_owned)
            .into_iter()
            .collect()
    })
}

/// `first` and its shorter forms, sanitized, without empty ones.
fn sanitized_forms(first: &str, shorter: &[String]) -> Vec<String> {
    std::iter::once(first)
        .chain(shorter.iter().map(String::as_str))
        .map(|form| sanitize_terminal_text(form).into_owned())
        .filter(|form| !form.is_empty())
        .collect()
}

/// A notice's first sanitized form within `room` cells, else its shortest form cut to the room,
/// so a notice is never dropped for width.
fn notice_form(forms: &[String], room: usize, theme: &Theme) -> Option<String> {
    forms
        .iter()
        .find(|form| display_width(form) <= room)
        .cloned()
        .or_else(|| cut_notice(forms.last()?, room, theme))
}

/// `form` cut at its end by display width, never inside a grapheme, and ended with the ellipsis;
/// `None` when not even one grapheme fits beside the ellipsis.
fn cut_notice(form: &str, room: usize, theme: &Theme) -> Option<String> {
    let ellipsis = theme.glyphs.ellipsis;
    let cut = truncate_display(form, room, ellipsis);
    (display_width(&cut) > display_width(ellipsis)).then_some(cut)
}

fn render_notice(frame: &mut Frame<'_>, area: Rect, notice: StatusNotice<'_>, theme: &Theme) {
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                format!("{} ", theme.glyphs.status_bullet),
                theme.style(notice.marker),
            ),
            Span::styled(
                sanitize_terminal_text(notice.text).into_owned(),
                theme.style(Role::TextPrimary),
            ),
        ])),
        area,
    );
}

fn render_indicator_and_hints(
    frame: &mut Frame<'_>,
    area: Rect,
    indicator: Option<&Indicator>,
    hints: &[OwnedHint],
    compact_hint_key: Option<&str>,
    separator: &Separator,
    theme: &Theme,
) {
    let mut x = area.x;
    if let Some(indicator) = indicator {
        let width = u16::try_from(indicator.width()).unwrap_or(u16::MAX);
        let rect = Rect::new(x, area.y, width.min(area.width), 1);
        let mut spans = vec![Span::styled(indicator.label, theme.style(indicator.role))];
        if let Some(key) = &indicator.key {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(key.clone(), theme.style(Role::HintKey)));
        }
        frame.render_widget(Paragraph::new(Line::from(spans)), rect);
        x = x.saturating_add(width);
        if !hints.is_empty() || compact_hint_key.is_some() {
            let width = u16::try_from(separator.width).unwrap_or(u16::MAX);
            frame.render_widget(
                Paragraph::new(separator.text.clone()).style(theme.style(Role::HintLabel)),
                Rect::new(x, area.y, width, 1),
            );
            x = x.saturating_add(width);
        }
    }
    if let Some(key) = compact_hint_key {
        frame.render_widget(
            Paragraph::new(Span::styled(key, theme.style(Role::HintKey))),
            Rect::new(
                x,
                area.y,
                u16::try_from(display_width(key)).unwrap_or(u16::MAX),
                1,
            ),
        );
        return;
    }
    if hints.is_empty() {
        return;
    }
    let borrowed = borrowed_hints(hints);
    let width = measure_key_hints(&borrowed, theme)
        .and_then(|width| u16::try_from(width).ok())
        .unwrap_or(0);
    let hint_area = Rect::new(x, area.y, width.min(area.right().saturating_sub(x)), 1);
    let _ = render_key_hints(frame, hint_area, &borrowed, theme);
}

fn borrowed_hints(hints: &[OwnedHint]) -> Vec<KeyHint<'_>> {
    hints
        .iter()
        .map(|hint| KeyHint::new(&hint.key, hint.label))
        .collect()
}

fn fitting_hints(hints: &[OwnedHint], available: usize, theme: &Theme) -> (usize, usize) {
    for count in (1..=hints.len()).rev() {
        let borrowed = borrowed_hints(&hints[..count]);
        if let Some(width) = measure_key_hints(&borrowed, theme)
            && width <= available
        {
            return (count, width);
        }
    }
    (0, 0)
}

/// Ordered hints and the order in which they yield to width.
#[derive(Debug, Default)]
struct HintSet {
    items: Vec<OwnedHint>,
    /// Indices in the order they drop; `None` drops from the end.
    yield_order: Option<Vec<usize>>,
}

impl From<Vec<OwnedHint>> for HintSet {
    fn from(items: Vec<OwnedHint>) -> Self {
        Self {
            items,
            yield_order: None,
        }
    }
}

/// Drops whole hints in yield order until the rest fit; kept hints stay in display order.
fn fit_hints(hints: &HintSet, available: usize, theme: &Theme) -> (Vec<OwnedHint>, usize) {
    let mut kept: Vec<usize> = (0..hints.items.len()).collect();
    let yield_order = hints
        .yield_order
        .clone()
        .unwrap_or_else(|| (0..hints.items.len()).rev().collect());
    let mut yielding = yield_order.into_iter();
    while !kept.is_empty() {
        let items = kept
            .iter()
            .map(|index| &hints.items[*index])
            .map(|hint| KeyHint::new(&hint.key, hint.label))
            .collect::<Vec<_>>();
        if let Some(width) = measure_key_hints(&items, theme)
            && width <= available
        {
            let kept = kept
                .iter()
                .map(|index| OwnedHint {
                    key: hints.items[*index].key.clone(),
                    label: hints.items[*index].label,
                })
                .collect();
            return (kept, width);
        }
        let Some(next) = yielding.next() else {
            break;
        };
        kept.retain(|index| *index != next);
    }
    (Vec::new(), 0)
}

fn input_indicator(
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    session_notice: bool,
) -> Option<InputIndicator> {
    if capture_owner == Some(CaptureOwner::Resize) {
        return Some(InputIndicator::Resize);
    }
    // The Find bar's own notices lead; the prefix stays live over the bar.
    if capture_owner == Some(CaptureOwner::Copy) && view.find_bar_active() {
        if view.prefix_active() {
            return Some(InputIndicator::Prefix);
        }
        return (!session_notice).then_some(InputIndicator::Find);
    }
    if capture_owner.is_none() && view.navigator_active() {
        return Some(InputIndicator::Navigator);
    }
    // The mobile list shows its own way back and Help, so it adds no navigation chip.
    if capture_owner.is_none()
        && view.machines.sidebar_navigation_active()
        && !view.sidebar_screen()
    {
        return Some(InputIndicator::Sidebar);
    }
    // A pending resize holds no input: a refusal of the person's next resize outranks it.
    if capture_owner.is_none() && view.resize_pending && !view.resize_notice_shown() {
        return Some(InputIndicator::ResizePending);
    }
    view.prefix_active().then_some(InputIndicator::Prefix)
}

fn indicator_projection(
    indicator: InputIndicator,
    view: &ClientView,
    locale: Locale,
    glyphs: KeyLabelGlyphs,
) -> Indicator {
    let copy = locale.status();
    match indicator {
        InputIndicator::Prefix => Indicator {
            label: copy.prefix,
            key: Some(format_binding_with_glyphs(
                &view.bindings().prefix,
                LabelStyle::for_host(),
                glyphs,
            )),
            role: Role::AccentFocus,
        },
        InputIndicator::Navigator => Indicator {
            label: copy.navigator,
            key: None,
            role: Role::AccentFocus,
        },
        InputIndicator::Sidebar => Indicator {
            label: copy.sidebar,
            key: None,
            role: Role::AccentFocus,
        },
        InputIndicator::Resize => Indicator {
            label: if view.resize_pending {
                copy.resize_pending
            } else {
                copy.resize
            },
            key: None,
            role: if view.resize_pending {
                Role::StatusWorking
            } else {
                Role::AccentFocus
            },
        },
        InputIndicator::ResizePending => Indicator {
            label: copy.resize_pending,
            key: None,
            role: Role::StatusWorking,
        },
        InputIndicator::Find => Indicator {
            label: locale.find_indicator(),
            key: None,
            role: Role::AccentFocus,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn status_hints(
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    indicator: Option<InputIndicator>,
    client_notice: Option<Notice>,
    session: Option<&WorkbenchClientView<'_>>,
    contexts: &[ContextIndicator],
    tier: ViewportTier,
    locale: Locale,
    glyphs: KeyLabelGlyphs,
) -> HintSet {
    let copy = locale.status();
    match indicator {
        Some(InputIndicator::Prefix) => {
            return prefix_mode_hints(view, locale, glyphs).into();
        }
        Some(InputIndicator::Navigator) => {
            use crate::client_view::navigator::Command;
            let nav = locale.navigator();
            return [
                (Command::Exit, nav.return_action),
                (Command::Open, nav.open_action),
            ]
            .into_iter()
            .map(|(command, label)| OwnedHint {
                key: crate::client_view::navigator::label_with_glyphs(command, glyphs),
                label,
            })
            .collect::<Vec<_>>()
            .into();
        }
        Some(InputIndicator::Sidebar) => return sidebar_hints(locale, glyphs),
        Some(InputIndicator::Resize) => return resize_hints(view, locale, glyphs).into(),
        Some(InputIndicator::ResizePending) => return HintSet::default(),
        Some(InputIndicator::Find) => {
            use crate::client_view::{KeyCode, Modifiers};
            let label = |code| {
                format_binding_with_glyphs(
                    &crate::terminal_events::keys::Binding::new(code, Modifiers::NONE),
                    LabelStyle::for_host(),
                    glyphs,
                )
            };
            return vec![
                OwnedHint {
                    key: label(KeyCode::Esc),
                    label: locale.find_close_label(),
                },
                OwnedHint {
                    key: label(KeyCode::Enter),
                    label: locale.find_next_label(),
                },
            ]
            .into();
        }
        None => {}
    }

    if capture_owner.is_some() {
        return HintSet::default();
    }
    if tier == ViewportTier::TooSmall {
        return first_effective_hint(
            view,
            PrefixAction::Client(ClientAction::Exit),
            copy.exit,
            glyphs,
        )
        .into_iter()
        .collect::<Vec<_>>()
        .into();
    }
    let mut hints = status_actions(client_notice, session)
        .into_iter()
        .filter_map(|action| action_hint(view, action, locale, glyphs))
        .collect::<Vec<_>>();
    for context in contexts {
        let (action, label) = match context {
            ContextIndicator::PointerReleased => (
                PrefixAction::Client(ClientAction::TogglePointerCapture),
                copy.restore,
            ),
            ContextIndicator::Zoomed => {
                (PrefixAction::Client(ClientAction::ToggleZoom), copy.restore)
            }
            ContextIndicator::Panels(HiddenPanels {
                toward: Some(direction),
                ..
            }) => (
                PrefixAction::Client(ClientAction::MovePanelFocus(*direction)),
                copy.switch_panel,
            ),
            ContextIndicator::Panels(_) => continue,
        };
        if let Some(hint) = effective_hint(view, action, label, glyphs) {
            hints.push(hint);
        }
    }
    // At rest nothing floats: there is no idle hint.
    hints.into()
}

/// Sidebar focus keeps its return and Help routes as width shrinks.
fn sidebar_hints(locale: Locale, glyphs: KeyLabelGlyphs) -> HintSet {
    use crate::machine_picker::{SidebarCommand, sidebar_binding_label};
    let commands = [
        SidebarCommand::Exit,
        SidebarCommand::Activate,
        SidebarCommand::OpenMenu,
        SidebarCommand::Help,
    ];
    let items = commands
        .into_iter()
        .filter_map(|command| {
            sidebar_binding_label(command, glyphs).map(|key| {
                (
                    command,
                    OwnedHint {
                        key,
                        label: locale.sidebar_navigation_action(command),
                    },
                )
            })
        })
        .collect::<Vec<_>>();
    let yield_order = [
        SidebarCommand::OpenMenu,
        SidebarCommand::Activate,
        SidebarCommand::Help,
        SidebarCommand::Exit,
    ]
    .into_iter()
    .filter_map(|command| items.iter().position(|(item, _)| *item == command))
    .collect();
    HintSet {
        items: items.into_iter().map(|(_, hint)| hint).collect(),
        yield_order: Some(yield_order),
    }
}

fn prefix_mode_hints(view: &ClientView, locale: Locale, glyphs: KeyLabelGlyphs) -> Vec<OwnedHint> {
    let copy = locale.status();
    let mut hints = Vec::with_capacity(8);
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::Client(ClientAction::DismissPrefix),
        copy.cancel,
        glyphs,
    ));
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::ShowHelp,
        copy.help,
        glyphs,
    ));
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::Client(ClientAction::FocusBusinessSidebar),
        copy.focus_sidebar,
        glyphs,
    ));
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::Client(ClientAction::OpenActionMenu),
        copy.actions,
        glyphs,
    ));
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::Client(ClientAction::CreateTab),
        copy.create_tab,
        glyphs,
    ));
    hints.extend(prefix_action_group_hint(
        view,
        [
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Left)),
            PrefixAction::Client(ClientAction::SplitPanel(PanelDirection::Down)),
        ],
        copy.split,
        glyphs,
    ));
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::BoardToggle,
        copy.agent_board,
        glyphs,
    ));
    hints.extend(prefix_action_hint(
        view,
        PrefixAction::Client(ClientAction::Exit),
        copy.detach,
        glyphs,
    ));
    hints
}

fn prefix_action_hint(
    view: &ClientView,
    action: PrefixAction,
    label: &'static str,
    glyphs: KeyLabelGlyphs,
) -> Option<OwnedHint> {
    prefix_action_keys(view, action, glyphs)
        .into_iter()
        .next()
        .map(|key| OwnedHint { key, label })
}

fn prefix_action_group_hint(
    view: &ClientView,
    actions: impl IntoIterator<Item = PrefixAction>,
    label: &'static str,
    glyphs: KeyLabelGlyphs,
) -> Option<OwnedHint> {
    let keys = actions
        .into_iter()
        .flat_map(|action| prefix_action_keys(view, action, glyphs))
        .collect::<Vec<_>>();
    (!keys.is_empty()).then(|| OwnedHint {
        key: join_key_labels(&keys, LabelStyle::for_host(), " / "),
        label,
    })
}

fn prefix_action_keys(
    view: &ClientView,
    action: PrefixAction,
    glyphs: KeyLabelGlyphs,
) -> Vec<String> {
    let prefix = format!(
        "{} ",
        format_binding_with_glyphs(&view.bindings().prefix, LabelStyle::for_host(), glyphs,)
    );
    view.bindings()
        .action_labels_with_glyphs(action, glyphs)
        .into_iter()
        .filter_map(|label| label.strip_prefix(prefix.as_str()).map(str::to_owned))
        .collect()
}

fn resize_hints(view: &ClientView, locale: Locale, glyphs: KeyLabelGlyphs) -> Vec<OwnedHint> {
    let copy = locale.status();
    let rows = view.resize_help_rows_with_glyphs(glyphs);
    let mut result = Vec::new();
    for wanted in [
        PrefixDesc::ResizeExit,
        PrefixDesc::ResizeCoarse,
        PrefixDesc::ResizeFine,
        PrefixDesc::ShowHelp,
    ] {
        if view.resize_pending && wanted != PrefixDesc::ResizeExit {
            continue;
        }
        if let Some((key, _)) = rows.iter().find(|(_, desc)| *desc == wanted) {
            let label = match wanted {
                PrefixDesc::ResizeExit => copy.exit,
                PrefixDesc::ResizeCoarse => copy.resize_coarse,
                PrefixDesc::ResizeFine => copy.resize_fine,
                PrefixDesc::ShowHelp => copy.help,
                _ => unreachable!("bounded resize status actions"),
            };
            result.push(OwnedHint {
                key: key.clone(),
                label,
            });
        }
    }
    result
}

fn effective_hint(
    view: &ClientView,
    action: PrefixAction,
    label: &'static str,
    glyphs: KeyLabelGlyphs,
) -> Option<OwnedHint> {
    let labels = view.bindings().action_labels_with_glyphs(action, glyphs);
    (!labels.is_empty()).then(|| OwnedHint {
        key: join_key_labels(&labels, LabelStyle::for_host(), " / "),
        label,
    })
}

fn first_effective_hint(
    view: &ClientView,
    action: PrefixAction,
    label: &'static str,
    glyphs: KeyLabelGlyphs,
) -> Option<OwnedHint> {
    view.bindings()
        .action_labels_with_glyphs(action, glyphs)
        .into_iter()
        .next()
        .map(|key| OwnedHint { key, label })
}

fn action_hint(
    view: &ClientView,
    action: StatusAction,
    locale: Locale,
    glyphs: KeyLabelGlyphs,
) -> Option<OwnedHint> {
    let copy = locale.status();
    let (action, label) = match action {
        StatusAction::ShowHelp => (PrefixAction::ShowHelp, copy.shortcuts),
        StatusAction::Exit => (PrefixAction::Client(ClientAction::Exit), copy.exit),
        StatusAction::CreateTab => (
            PrefixAction::Client(ClientAction::CreateTab),
            copy.create_tab,
        ),
        // A host without Agent launch never advertises it.
        StatusAction::StartAgent
            if !view
                .capabilities()
                .supports(crate::tui_capabilities::Capability::AgentLaunch) =>
        {
            return None;
        }
        StatusAction::StartAgent => (
            PrefixAction::Client(ClientAction::StartAgent),
            copy.start_agent,
        ),
        StatusAction::CreateWorkspace => (
            PrefixAction::Client(ClientAction::CreateTab),
            copy.create_workspace,
        ),
        StatusAction::Refresh => (PrefixAction::NavigatorToggle, copy.refresh),
        StatusAction::CheckAgain => (PrefixAction::NavigatorToggle, copy.check_again),
        StatusAction::RetryClose => (
            PrefixAction::Client(ClientAction::RequestCloseTab),
            copy.retry_close,
        ),
        StatusAction::TakeControl => (
            PrefixAction::Client(ClientAction::RequestControlTakeover),
            copy.take_control,
        ),
        StatusAction::RetryRename(target) => (
            PrefixAction::Client(match target {
                LocationLabelTarget::Project => ClientAction::RequestProjectRename,
                LocationLabelTarget::Workspace => ClientAction::RequestWorkspaceRename,
            }),
            copy.retry_rename,
        ),
    };
    effective_hint(view, action, label, glyphs)
}

fn status_actions(
    client_notice: Option<Notice>,
    session: Option<&WorkbenchClientView<'_>>,
) -> Vec<StatusAction> {
    if client_notice == Some(Notice::UnknownPrefix) {
        return vec![StatusAction::ShowHelp];
    }
    if client_notice.is_some() {
        return Vec::new();
    }
    let Some(native) = session else {
        return Vec::new();
    };
    native_status_actions(native)
}

fn native_status_actions(native: &WorkbenchClientView<'_>) -> Vec<StatusAction> {
    if native.workspace_close_status().is_some() {
        return Vec::new();
    }
    if let Some(close) = native.announced_close_status() {
        let mut actions = Vec::with_capacity(2);
        if close.unavailable() || close.observation_paused() {
            actions.push(StatusAction::CheckAgain);
        }
        if close.resumable() {
            actions.push(StatusAction::RetryClose);
        }
        return actions;
    }
    if native.worktree_notice().is_some() {
        return Vec::new();
    }
    if let Some((status, _)) = native.takeover_status() {
        return if matches!(status, TakeoverStatus::Refused | TakeoverStatus::Lost)
            && !focus_terminal_ended(native)
        {
            vec![StatusAction::TakeControl]
        } else {
            Vec::new()
        };
    }
    if native.notice().is_none()
        && native.retirement().is_none()
        && native.setup_status().is_none()
        && native.duplicate_progress().is_some()
    {
        return Vec::new();
    }

    let phase_actions = native_phase_actions(native);
    if native.retirement().is_some() {
        return phase_actions;
    }
    if native.notice().is_some() {
        return match native.setup_status() {
            Some(SetupStatus::LocationLabel {
                status: LocationLabelStatus::CommittedNeedRefresh,
                ..
            })
            | Some(SetupStatus::CommittedNeedRefresh | SetupStatus::TabsCommittedNeedRefresh) => {
                vec![StatusAction::Refresh]
            }
            _ => Vec::new(),
        };
    }
    match native.setup_status() {
        Some(SetupStatus::DirectoryAccepted) => vec![StatusAction::CreateTab],
        // The refused text is retained for this owner; the retry stays a separate,
        // droppable hint so the short refusal fact keeps priority.
        Some(SetupStatus::LocationLabel {
            target,
            status: LocationLabelStatus::Stale,
        }) => vec![StatusAction::RetryRename(target)],
        Some(SetupStatus::LocationLabel {
            status: LocationLabelStatus::CommittedNeedRefresh,
            ..
        })
        | Some(SetupStatus::CommittedNeedRefresh | SetupStatus::TabsCommittedNeedRefresh) => {
            vec![StatusAction::Refresh]
        }
        Some(_) => Vec::new(),
        None => phase_actions,
    }
}

fn native_phase_actions(native: &WorkbenchClientView<'_>) -> Vec<StatusAction> {
    if native.quick_workspace_recovery().is_some()
        || (native.phase() == DisplayPhase::SelectedWithoutAttachment
            && native.selected_is_unbound()
            && native.retirement().is_none())
    {
        return Vec::new();
    }
    if native.retirement().is_some() {
        return vec![StatusAction::Exit];
    }
    match native.phase() {
        DisplayPhase::Empty
            if native.workspace_directory_path().is_some()
                || native.workspace_directory_unconfigured() =>
        {
            Vec::new()
        }
        DisplayPhase::Empty if native.no_current_workspace() => {
            vec![StatusAction::CreateWorkspace]
        }
        DisplayPhase::Empty => vec![StatusAction::CreateTab, StatusAction::StartAgent],
        DisplayPhase::Ready if native.active_position().is_none() => {
            vec![StatusAction::Exit]
        }
        // An ended terminal keeps its output but accepts no controller.
        DisplayPhase::Ready if focus_terminal_ended(native) => {
            vec![StatusAction::CreateTab]
        }
        DisplayPhase::Ready if native.controller() == Some(DisplayController::ReadOnly) => {
            vec![StatusAction::TakeControl]
        }
        // A known failed launch is never retried; another Tab is the direct recovery.
        DisplayPhase::SelectedWithoutAttachment
            if native.active_position().is_some()
                && native.selected_provenance() == Some(DisplayProvenance::Failed) =>
        {
            vec![StatusAction::CreateTab]
        }
        DisplayPhase::ProjectionUnavailable
        | DisplayPhase::MoveProjectionUnavailable
        | DisplayPhase::PanelProjectionUnavailable
        | DisplayPhase::RenameProjectionUnavailable => vec![StatusAction::Refresh],
        DisplayPhase::Switching
        | DisplayPhase::SelectedWithoutAttachment
        | DisplayPhase::RetryableAttachUnavailable
            if native.active_position().is_none() =>
        {
            vec![StatusAction::Exit]
        }
        DisplayPhase::Retired => vec![StatusAction::Exit],
        _ => Vec::new(),
    }
}

fn focus_terminal_ended(native: &WorkbenchClientView<'_>) -> bool {
    native
        .focus_panel_id()
        .is_some_and(|panel| native.terminal_end(panel).is_some())
}

#[cfg(test)]
mod tests;
