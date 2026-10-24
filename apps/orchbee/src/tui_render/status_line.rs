//! Typed projection and painting for the shell status line.

#[cfg(windows)]
use crate::client_view::PrefixDesc;
use crate::{
    client_view::{
        ClientAction, ClientView, KeyLabelGlyphs, LabelStyle, Notice, PanelDirection, PrefixAction,
        format_binding_with_glyphs, join_key_labels,
    },
    localization::Locale,
    terminal_session::{
        NativeDisplayController, NativeDisplayPhase, NativeLocationLabelStatus,
        NativeLocationLabelTarget, NativeSessionView, NativeSetupStatus, TakeoverStatus,
        TerminalSessionView,
    },
    terminal_text::{display_width, sanitize_terminal_text},
    tui_composition::CaptureOwner,
    tui_layout::ViewportTier,
    tui_theme::{GlyphMode, Role, Theme},
};
use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use super::controls::{KeyHint, measure_key_hints, render_key_hints};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InputIndicator {
    Prefix,
    #[cfg(windows)]
    Navigator,
    #[cfg(windows)]
    Sidebar,
    #[cfg(windows)]
    Resize,
    #[cfg(windows)]
    ResizePending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ContextIndicator {
    PointerReleased,
    Zoomed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StatusAction {
    ShowHelp,
    Exit,
    CreateTab,
    #[cfg(windows)]
    CreateWorkspace,
    Refresh,
    CheckAgain,
    RetryClose,
    TakeControl,
    ConfigureDirectory,
    RetryRename(NativeLocationLabelTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StatusNotice<'a> {
    text: &'a str,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    locale: Locale,
    status: &str,
    secondary_notice: Option<&str>,
    zoomed: bool,
    read_only: bool,
    recovery_status: bool,
    tier: ViewportTier,
    theme: &Theme,
    session: Option<&TerminalSessionView<'_>>,
) -> Option<Rect> {
    if area.is_empty() {
        return None;
    }

    let client_notice = view
        .notice()
        .filter(|notice| !(read_only && *notice == Notice::TerminalInputUnavailable));
    let primary = client_notice.map_or(status, |notice| locale.shell_notice(Some(notice)));
    let secondary_notice = secondary_notice.filter(|notice| {
        !view.prefix_active()
            && !primary
                .strip_prefix(*notice)
                .is_some_and(|suffix| suffix.is_empty() || suffix.starts_with(" · "))
    });
    let glyphs = key_label_glyphs(theme);
    let indicator_kind = input_indicator(view, capture_owner);
    let mut contexts = Vec::with_capacity(2);
    if view.pointer_released() {
        contexts.push(ContextIndicator::PointerReleased);
    }
    if zoomed {
        contexts.push(ContextIndicator::Zoomed);
    }

    let notice = (indicator_kind.is_none())
        .then_some(secondary_notice)
        .flatten()
        .map(|text| StatusNotice {
            text,
            marker: Role::StatusWarning,
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
    } else if let Some(notice) = notice {
        let width = notice_width(notice, theme);
        if width <= usize::from(area.width) {
            right_width = width;
            right_is_notice = true;
        }
    } else if tier == ViewportTier::TooSmall && !hints.is_empty() {
        let (count, width) = fitting_hints(&hints, usize::from(area.width), theme);
        visible_hints = count;
        right_width = width;
    } else if primary_width <= usize::from(area.width) && !hints.is_empty() {
        let remaining = usize::from(area.width)
            .saturating_sub(primary_width)
            .saturating_sub(separator.width);
        let (count, width) = fitting_hints(&hints, remaining, theme);
        visible_hints = count;
        right_width = width;
    }

    if hide_indicator {
        indicator = None;
    }

    if recovery_status
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
    if primary_visible {
        render_primary(frame, left, &primary, &contexts, locale, theme);
    }

    if right_width_u16 > 0 {
        let right = Rect::new(area.right() - right_width_u16, area.y, right_width_u16, 1);
        if primary_visible && left_gap > 0 {
            frame.buffer_mut().set_string(
                right.x - 1,
                right.y,
                " ",
                theme.style(Role::SurfaceChrome),
            );
        }
        if right_is_notice {
            if let Some(notice) = notice {
                render_notice(frame, right, notice, theme);
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

    if client_notice.is_none() {
        primary_rect
    } else {
        None
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
    let copy = locale.status();
    let mut spans = vec![Span::styled(
        primary.to_owned(),
        theme.style(Role::TextPrimary),
    )];
    let mut used = display_width(primary);
    let separator = separator(theme);
    for context in contexts {
        let label = match context {
            ContextIndicator::PointerReleased => copy.pointer_released,
            ContextIndicator::Zoomed => copy.zoomed,
        };
        let next = used
            .saturating_add(separator.width)
            .saturating_add(display_width(label));
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
    display_width(theme.glyphs.status_bullet)
        .saturating_add(1)
        .saturating_add(display_width(&sanitize_terminal_text(notice.text)))
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

fn key_label_glyphs(theme: &Theme) -> KeyLabelGlyphs {
    match theme.glyphs.mode() {
        GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(test)]
        GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    }
}

fn input_indicator(
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
) -> Option<InputIndicator> {
    #[cfg(windows)]
    if capture_owner == Some(CaptureOwner::Resize) {
        return Some(InputIndicator::Resize);
    }
    #[cfg(windows)]
    if capture_owner.is_none() && view.navigator_active() {
        return Some(InputIndicator::Navigator);
    }
    #[cfg(windows)]
    if capture_owner.is_none() && view.machines.sidebar_navigation_active() {
        return Some(InputIndicator::Sidebar);
    }
    #[cfg(windows)]
    if capture_owner.is_none() && view.resize_pending {
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
        #[cfg(windows)]
        InputIndicator::Navigator => Indicator {
            label: copy.navigator,
            key: None,
            role: Role::AccentFocus,
        },
        #[cfg(windows)]
        InputIndicator::Sidebar => Indicator {
            label: copy.sidebar,
            key: None,
            role: Role::AccentFocus,
        },
        #[cfg(windows)]
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
        #[cfg(windows)]
        InputIndicator::ResizePending => Indicator {
            label: copy.resize_pending,
            key: None,
            role: Role::StatusWorking,
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn status_hints(
    view: &ClientView,
    capture_owner: Option<CaptureOwner>,
    indicator: Option<InputIndicator>,
    client_notice: Option<Notice>,
    session: Option<&TerminalSessionView<'_>>,
    contexts: &[ContextIndicator],
    tier: ViewportTier,
    locale: Locale,
    glyphs: KeyLabelGlyphs,
) -> Vec<OwnedHint> {
    let copy = locale.status();
    match indicator {
        Some(InputIndicator::Prefix) => {
            return prefix_mode_hints(view, locale, glyphs);
        }
        #[cfg(windows)]
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
            .collect();
        }
        #[cfg(windows)]
        Some(InputIndicator::Sidebar) => {
            use crate::machine_picker::{SidebarCommand, sidebar_binding_label};
            let mut commands = vec![SidebarCommand::Exit];
            if tier == ViewportTier::Full {
                commands.extend([
                    SidebarCommand::Activate,
                    SidebarCommand::OpenMenu,
                    SidebarCommand::Help,
                ]);
            }
            return commands
                .into_iter()
                .filter_map(|command| {
                    sidebar_binding_label(command, glyphs).map(|key| OwnedHint {
                        key,
                        label: locale.sidebar_navigation_action(command),
                    })
                })
                .collect();
        }
        #[cfg(windows)]
        Some(InputIndicator::Resize) => return resize_hints(view, locale, glyphs),
        #[cfg(windows)]
        Some(InputIndicator::ResizePending) => return Vec::new(),
        None => {}
    }

    if capture_owner.is_some() {
        return Vec::new();
    }
    if tier == ViewportTier::TooSmall {
        return first_effective_hint(
            view,
            PrefixAction::Client(ClientAction::Exit),
            copy.exit,
            glyphs,
        )
        .into_iter()
        .collect();
    }
    let mut hints = status_actions(client_notice, session)
        .into_iter()
        .filter_map(|action| action_hint(view, action, locale, glyphs))
        .collect::<Vec<_>>();
    for context in contexts {
        let action = match context {
            ContextIndicator::PointerReleased => {
                PrefixAction::Client(ClientAction::TogglePointerCapture)
            }
            ContextIndicator::Zoomed => PrefixAction::Client(ClientAction::ToggleZoom),
        };
        if let Some(hint) = effective_hint(view, action, copy.restore, glyphs) {
            hints.push(hint);
        }
    }
    if !hints.is_empty() {
        return hints;
    }
    match tier {
        ViewportTier::Full => vec![OwnedHint {
            key: format_binding_with_glyphs(
                &view.bindings().prefix,
                LabelStyle::for_host(),
                glyphs,
            ),
            label: copy.shortcuts,
        }],
        ViewportTier::Compact => effective_hint(view, PrefixAction::ShowHelp, copy.help, glyphs)
            .into_iter()
            .collect(),
        ViewportTier::Minimal => effective_hint(
            view,
            PrefixAction::Client(ClientAction::Exit),
            copy.exit,
            glyphs,
        )
        .into_iter()
        .collect(),
        ViewportTier::TooSmall => first_effective_hint(
            view,
            PrefixAction::Client(ClientAction::Exit),
            copy.exit,
            glyphs,
        )
        .into_iter()
        .collect(),
    }
}

fn prefix_mode_hints(view: &ClientView, locale: Locale, glyphs: KeyLabelGlyphs) -> Vec<OwnedHint> {
    let copy = locale.status();
    let mut hints = Vec::with_capacity(7);
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
    #[cfg(windows)]
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

#[cfg(windows)]
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
        #[cfg(windows)]
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
        StatusAction::ConfigureDirectory => (
            PrefixAction::Client(ClientAction::RequestWorkspaceDirectory),
            copy.configure_directory,
        ),
        StatusAction::RetryRename(target) => (
            PrefixAction::Client(match target {
                NativeLocationLabelTarget::Project => ClientAction::RequestProjectRename,
                NativeLocationLabelTarget::Workspace => ClientAction::RequestWorkspaceRename,
            }),
            copy.retry_rename,
        ),
    };
    effective_hint(view, action, label, glyphs)
}

fn status_actions(
    client_notice: Option<Notice>,
    session: Option<&TerminalSessionView<'_>>,
) -> Vec<StatusAction> {
    if client_notice == Some(Notice::UnknownPrefix) {
        return vec![StatusAction::ShowHelp];
    }
    if client_notice.is_some() {
        return Vec::new();
    }
    let Some(TerminalSessionView::Native(native)) = session else {
        return Vec::new();
    };
    native_status_actions(native)
}

fn native_status_actions(native: &NativeSessionView<'_>) -> Vec<StatusAction> {
    if native.workspace_close_status().is_some() {
        return Vec::new();
    }
    if let Some(close) = native.close_status() {
        let mut actions = Vec::with_capacity(2);
        if close.unavailable() || close.observation_paused() {
            actions.push(StatusAction::CheckAgain);
        }
        if close.resumable() {
            actions.push(StatusAction::RetryClose);
        }
        return actions;
    }
    #[cfg(windows)]
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
    #[cfg(windows)]
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
            Some(NativeSetupStatus::LocationLabel {
                status: NativeLocationLabelStatus::CommittedNeedRefresh,
                ..
            })
            | Some(
                NativeSetupStatus::CommittedNeedRefresh
                | NativeSetupStatus::TabsCommittedNeedRefresh,
            ) => vec![StatusAction::Refresh],
            _ => Vec::new(),
        };
    }
    match native.setup_status() {
        Some(NativeSetupStatus::WorkspaceCreated) if native.workspace_directory_unconfigured() => {
            vec![StatusAction::ConfigureDirectory]
        }
        Some(NativeSetupStatus::DirectoryAccepted) => vec![StatusAction::CreateTab],
        // The refused text is retained for this owner; the retry stays a separate,
        // droppable hint so the short refusal fact keeps priority.
        Some(NativeSetupStatus::LocationLabel {
            target,
            status: NativeLocationLabelStatus::Stale,
        }) => vec![StatusAction::RetryRename(target)],
        Some(NativeSetupStatus::LocationLabel {
            status: NativeLocationLabelStatus::CommittedNeedRefresh,
            ..
        })
        | Some(
            NativeSetupStatus::CommittedNeedRefresh | NativeSetupStatus::TabsCommittedNeedRefresh,
        ) => vec![StatusAction::Refresh],
        Some(_) => Vec::new(),
        None => phase_actions,
    }
}

fn native_phase_actions(native: &NativeSessionView<'_>) -> Vec<StatusAction> {
    if native.quick_workspace_recovery().is_some()
        || (native.phase() == NativeDisplayPhase::SelectedWithoutAttachment
            && native.selected_is_unbound()
            && native.retirement().is_none())
    {
        return Vec::new();
    }
    if native.retirement().is_some() {
        return vec![StatusAction::Exit];
    }
    match native.phase() {
        NativeDisplayPhase::Empty
            if native.workspace_directory_path().is_some()
                || native.workspace_directory_unconfigured() =>
        {
            Vec::new()
        }
        #[cfg(windows)]
        NativeDisplayPhase::Empty if native.no_current_workspace() => {
            vec![StatusAction::CreateWorkspace]
        }
        NativeDisplayPhase::Empty => vec![StatusAction::CreateTab],
        NativeDisplayPhase::Ready if native.active_position().is_none() => {
            vec![StatusAction::Exit]
        }
        // An ended terminal keeps its output but accepts no controller.
        NativeDisplayPhase::Ready if focus_terminal_ended(native) => {
            vec![StatusAction::CreateTab]
        }
        NativeDisplayPhase::Ready
            if native.controller() == Some(NativeDisplayController::ReadOnly) =>
        {
            vec![StatusAction::TakeControl]
        }
        NativeDisplayPhase::ProjectionUnavailable
        | NativeDisplayPhase::MoveProjectionUnavailable
        | NativeDisplayPhase::PanelProjectionUnavailable
        | NativeDisplayPhase::RenameProjectionUnavailable => vec![StatusAction::Refresh],
        NativeDisplayPhase::Switching
        | NativeDisplayPhase::SelectedWithoutAttachment
        | NativeDisplayPhase::RetryableAttachUnavailable
            if native.active_position().is_none() =>
        {
            vec![StatusAction::Exit]
        }
        NativeDisplayPhase::Retired => vec![StatusAction::Exit],
        _ => Vec::new(),
    }
}

fn focus_terminal_ended(native: &NativeSessionView<'_>) -> bool {
    native
        .focus_panel_id()
        .is_some_and(|panel| native.terminal_end(panel).is_some())
}

#[cfg(test)]
mod tests;
