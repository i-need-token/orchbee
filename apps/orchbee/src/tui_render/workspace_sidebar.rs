//! Workspace token rows over complete canonical checkpoints and attention-projected Agent states.
use super::*;
use crate::{machine_discovery::display_label, machine_picker::Row};
use orchbee_local_site_runtime::{WindowsSidebarTokenStyle, WindowsSpaceSidebarTokenKind as Kind};
use sidebar_text::{fit, machine_label, state_role};
struct Context<'a> {
    row: &'a Row,
    observed: Option<&'a crate::machine_spaces::Row>,
    state: &'static str,
    role: Role,
    standalone: &'static str,
}
impl<'a> Context<'a> {
    fn new(view: &'a ClientView, row: &'a Row, locale: Locale) -> Self {
        let (key, role) = state_role(view.machines.workspace_state(row));
        Self {
            row,
            observed: view.machines.space_checkpoint_row(row),
            state: locale.machine_picker_text(key),
            role,
            standalone: locale.shell().standalone,
        }
    }
    fn value(&self, kind: Kind<'_>) -> Option<&str> {
        match kind {
            Kind::StateIcon | Kind::GitStatus => None,
            Kind::StateText => Some(self.state),
            Kind::Workspace => Some(
                self.observed
                    .and_then(|r| r.label.as_deref())
                    .or_else(|| self.row.workspace_label())
                    .unwrap_or(self.standalone),
            ),
            Kind::Branch => self.observed?.git.as_ref()?.branch.as_deref(),
            Kind::Custom(key) => self
                .observed?
                .tokens
                .as_map()
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v.as_str()),
        }
    }
    fn visible(&self, kind: Kind<'_>, style: Option<WindowsSidebarTokenStyle>) -> bool {
        if style.is_none() {
            return false;
        }
        match kind {
            Kind::StateIcon => true,
            Kind::GitStatus => self
                .observed
                .and_then(|r| r.git.as_ref())
                .and_then(|g| g.divergence.as_ref())
                .is_some_and(|g| g.ahead > 0 || g.behind > 0),
            _ => self
                .value(kind)
                .is_some_and(|v| !display_label(v).is_empty()),
        }
    }
}
fn git_text(context: &Context<'_>, theme: &Theme) -> String {
    let Some(git) = context
        .observed
        .and_then(|r| r.git.as_ref())
        .and_then(|g| g.divergence.as_ref())
    else {
        return String::new();
    };
    let mut parts = Vec::new();
    if git.ahead > 0 {
        parts.push(format!("{}{n}", theme.glyphs.git_ahead, n = git.ahead));
    }
    if git.behind > 0 {
        parts.push(format!("{}{n}", theme.glyphs.git_behind, n = git.behind));
    }
    parts.join(" ")
}
fn lines(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let context = Context::new(view, row, locale);
    let current = view.machines.current_workspace(row);
    let marker = if current {
        theme.glyphs.active_marker
    } else {
        " "
    };
    let machine = if view.machines.rows.iter().filter(|m| m.enabled).count() > 1 {
        format!(
            "{} / ",
            truncate_display(&machine_label(view, row, locale), 8, theme.glyphs.ellipsis)
        )
    } else {
        String::new()
    };
    let identity = format!("{marker}{machine}");
    let mut lines = Vec::new();
    for tokens in view.machines.space_settings().rows() {
        let resolved: Vec<_> = tokens
            .iter()
            .filter_map(|token| {
                let kind = token.kind();
                let style = token.style_for_value(context.value(kind));
                if !context.visible(kind, style) {
                    return None;
                }
                let text = match kind {
                    Kind::StateIcon => theme.glyphs.status_bullet.to_owned(),
                    Kind::GitStatus => git_text(&context, theme),
                    _ => display_label(context.value(kind).expect("visible value")),
                };
                Some((kind, style.expect("visible style"), text))
            })
            .collect();
        let desired: Vec<_> = resolved
            .iter()
            .map(|(_, _, text)| display_width(text))
            .collect();
        let separators: usize = resolved
            .windows(2)
            .map(|pair| if pair[0].0 == Kind::StateIcon { 1 } else { 3 })
            .sum();
        let fixed: usize = resolved
            .iter()
            .filter(|(kind, _, _)| matches!(kind, Kind::StateIcon | Kind::GitStatus))
            .map(|(_, _, text)| display_width(text))
            .sum();
        let mut remaining = width
            .saturating_sub(if lines.is_empty() {
                display_width(&identity)
            } else {
                0
            })
            .saturating_sub(fixed + separators);
        let mut budgets = vec![0; resolved.len()];
        // Distribute bounded text cells fairly; fixed counts keep their intrinsic width.
        loop {
            let mut assigned = false;
            for (i, (kind, _, _)) in resolved.iter().enumerate() {
                if remaining > 0
                    && !matches!(kind, Kind::StateIcon | Kind::GitStatus)
                    && budgets[i] < desired[i]
                {
                    budgets[i] += 1;
                    remaining -= 1;
                    assigned = true;
                }
            }
            if !assigned {
                break;
            }
        }
        let mut spans = Vec::new();
        let mut previous_icon = false;
        for (i, (kind, style, text)) in resolved.into_iter().enumerate() {
            let fixed = matches!(kind, Kind::StateIcon | Kind::GitStatus);
            let text = if fixed {
                text
            } else {
                truncate_display(&text, budgets[i], theme.glyphs.ellipsis)
            };
            if text.is_empty() {
                continue;
            }
            if !spans.is_empty() {
                spans.push(Span::styled(
                    if previous_icon { " " } else { " · " },
                    theme.style(Role::TextSecondary),
                ));
            }
            previous_icon = kind == Kind::StateIcon;
            if kind == Kind::GitStatus {
                let git = context
                    .observed
                    .and_then(|r| r.git.as_ref())
                    .and_then(|g| g.divergence.as_ref())
                    .expect("visible counts");
                if git.ahead > 0 {
                    spans.push(Span::styled(
                        format!("{}{n}", theme.glyphs.git_ahead, n = git.ahead),
                        theme.sidebar_token_style(Role::StatusSuccess, style),
                    ));
                }
                if git.ahead > 0 && git.behind > 0 {
                    spans.push(Span::raw(" "));
                }
                if git.behind > 0 {
                    spans.push(Span::styled(
                        format!("{}{n}", theme.glyphs.git_behind, n = git.behind),
                        theme.sidebar_token_style(Role::StatusError, style),
                    ));
                }
            } else {
                let role = match kind {
                    Kind::StateIcon | Kind::StateText => context.role,
                    Kind::Branch => Role::TextSecondary,
                    _ => Role::TextPrimary,
                };
                spans.push(Span::styled(text, theme.sidebar_token_style(role, style)));
            }
        }
        // A visible configured row keeps its height even when a tiny viewport clips all text.
        if !spans.is_empty()
            || !tokens.is_empty()
                && tokens.iter().any(|token| {
                    context.visible(
                        token.kind(),
                        token.style_for_value(context.value(token.kind())),
                    )
                })
        {
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    lines[0].spans.insert(
        0,
        Span::styled(
            identity,
            theme.style(if current {
                Role::AccentFocus
            } else {
                Role::HintKey
            }),
        ),
    );
    lines
}

fn height(view: &ClientView, row: &Row, locale: Locale) -> u16 {
    let context = Context::new(view, row, locale);
    view.machines
        .space_settings()
        .rows()
        .iter()
        .filter(|tokens| {
            tokens.iter().any(|token| {
                context.visible(
                    token.kind(),
                    token.style_for_value(context.value(token.kind())),
                )
            })
        })
        .count()
        .max(1) as u16
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    view: &ClientView,
    locale: Locale,
    theme: &Theme,
    layer: HitLayer,
    mut hits: Option<&mut HitMapBuilder>,
) {
    if area.is_empty() {
        return;
    }
    let title = format!(
        "{} {}",
        locale.shell().workspaces,
        view.machines.space_rows().len()
    );
    let header = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(truncate_display(
            &title,
            usize::from(area.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)),
        header,
    );
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, header, HitTarget::OpenMachineWorkspaces);
    }
    if area.height < 3 {
        return;
    }
    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 2);
    let rows = view.machines.space_rows();
    let heights: Vec<_> = rows.iter().map(|row| height(view, row, locale)).collect();
    let gap = view.machines.space_settings().row_gap();
    let window = view
        .machines
        .space_window_for_heights(&heights, body.height, gap);
    let mut y = body.y;
    for (index, row) in rows
        .iter()
        .enumerate()
        .skip(window.top)
        .take(window.capacity)
    {
        let rect = Rect::new(body.x, y, body.width, heights[index].min(body.height));
        frame.render_widget(
            Paragraph::new(
                lines(view, row, locale, theme, usize::from(body.width))
                    .into_iter()
                    .map(|line| fit(line, usize::from(body.width), theme.glyphs.ellipsis))
                    .collect::<Vec<_>>(),
            )
            .style(theme.style(Role::TextPrimary)),
            rect,
        );
        if let Some(hits) = hits.as_deref_mut() {
            hits.record(
                layer,
                rect,
                if row.enabled {
                    HitTarget::MachineWorkspaceRow {
                        selected: row.selection(),
                        window,
                    }
                } else {
                    HitTarget::MachineWorkspaceSurface(window)
                },
            );
        }
        y = rect.bottom();
        if index + 1 < window.end() {
            let blank = Rect::new(
                body.x,
                y,
                body.width,
                gap.min(body.bottom().saturating_sub(y)),
            );
            if let Some(hits) = hits.as_deref_mut() {
                hits.record(layer, blank, HitTarget::MachineWorkspaceSurface(window));
            }
            y = blank.bottom();
        }
    }
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(
            layer,
            Rect::new(body.x, y, body.width, body.bottom().saturating_sub(y)),
            HitTarget::MachineWorkspaceSurface(window),
        );
    }
    let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    let range = format!(
        "{}-{}/{}",
        if window.capacity > 0 {
            window.top + 1
        } else {
            0
        },
        window.end(),
        window.total
    );
    let count_width = display_width(&range).min(usize::from(footer.width)) as u16;
    frame.render_widget(
        Paragraph::new(truncate_display(
            locale.machine_picker_text("workspace-sidebar-open"),
            usize::from(footer.width.saturating_sub(count_width + 1)),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::HintKey)),
        Rect::new(
            footer.x,
            footer.y,
            footer.width.saturating_sub(count_width + 1),
            1,
        ),
    );
    frame.render_widget(
        Paragraph::new(range)
            .alignment(Alignment::Right)
            .style(theme.style(Role::TextSecondary)),
        Rect::new(footer.right() - count_width, footer.y, count_width, 1),
    );
    if let Some(hits) = hits {
        hits.record(layer, footer, HitTarget::OpenMachineWorkspaces);
    }
}
#[cfg(test)]
mod tests;
