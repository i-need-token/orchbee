//! Workspace token rows over complete canonical checkpoints and attention-projected Agent states.
use super::*;
use crate::{
    machine_discovery::display_label,
    machine_picker::{Row, SidebarRow, SidebarTarget},
};
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
    suffix: &str,
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
                display_width(&identity) + display_width(suffix)
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
    lines[0] = fit(
        std::mem::take(&mut lines[0]),
        width.saturating_sub(display_width(suffix)),
        theme.glyphs.ellipsis,
    );
    lines[0].spans.push(Span::styled(
        suffix.to_owned(),
        theme.style(Role::TextSecondary),
    ));
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

fn identity(target: &SidebarTarget) -> &str {
    match target {
        SidebarTarget::Project { project_id, .. } => project_id.as_str(),
        SidebarTarget::Context { target, .. } => match &target.location {
            crate::terminal_session::NativeContextLocation::Standalone(id) => id.as_str(),
            crate::terminal_session::NativeContextLocation::Workspace { workspace_id, .. } => {
                workspace_id.as_str()
            }
        },
    }
}

fn label(row: &SidebarRow<'_>, locale: Locale) -> String {
    display_label(match row {
        SidebarRow::Project { label, .. } => {
            if label.is_empty() {
                locale.shell().projects
            } else {
                label
            }
        }
        SidebarRow::Context(row) => row.workspace_label().unwrap_or(locale.shell().standalone),
    })
}

fn annotations(rows: &[SidebarRow<'_>], locale: Locale) -> Vec<String> {
    let mut groups = std::collections::BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        let target = row.target();
        let machine = match target.machine() {
            crate::machine_picker::Choice::Local => None,
            crate::machine_picker::Choice::Remote(id) => Some(id.as_str().to_owned()),
        };
        let parent = target.parent().map(|parent| identity(&parent).to_owned());
        groups
            .entry((
                machine,
                target.site_id().as_str().to_owned(),
                parent,
                matches!(row, SidebarRow::Project { .. }),
                label(row, locale),
            ))
            .or_insert_with(Vec::new)
            .push((index, identity(&target).to_owned()));
    }
    let mut annotations = vec![String::new(); rows.len()];
    for peers in groups.values().filter(|peers| peers.len() > 1) {
        let mut counts = std::collections::BTreeMap::new();
        for (_, id) in peers {
            for length in (8..=32).step_by(4) {
                *counts.entry(&id[id.len() - length..]).or_insert(0) += 1;
            }
        }
        for (index, id) in peers {
            let length = (8..=32)
                .step_by(4)
                .find(|length| counts[&id[id.len() - length..]] == 1)
                .unwrap_or(32);
            annotations[*index] = format!(" [{}]", &id[id.len() - length..]);
        }
    }
    annotations
}

fn menu_width(view: &ClientView, target: &SidebarTarget, theme: &Theme) -> u16 {
    if target.machine() == &view.machines.current
        && view
            .machines
            .current_context
            .as_ref()
            .is_some_and(|context| &context.site_id == target.site_id())
    {
        // The glyph is a menu control, not a localized label or an identity.
        u16::try_from(display_width(theme.glyphs.ellipsis) + 2).unwrap_or(0)
    } else {
        0
    }
}

fn entry_width(view: &ClientView, row: &SidebarRow<'_>, width: u16, theme: &Theme) -> usize {
    let indent = match row {
        SidebarRow::Project { .. } => 4,
        SidebarRow::Context(row) => {
            u16::from(
                row.sidebar_target()
                    .and_then(|target| target.parent())
                    .is_some(),
            ) * 2
        }
    };
    usize::from(width.saturating_sub(menu_width(view, &row.target(), theme) + indent))
}

fn suffix_rows(suffix: &str, width: usize) -> usize {
    if display_width(suffix) <= width.saturating_sub(4) {
        0
    } else {
        suffix.trim_start().len().div_ceil(width.max(1))
    }
}

fn entry_height(
    view: &ClientView,
    row: &SidebarRow<'_>,
    suffix: &str,
    locale: Locale,
    width: u16,
    theme: &Theme,
) -> u16 {
    let base = match row {
        SidebarRow::Project { .. } => 1,
        SidebarRow::Context(row) => height(view, row, locale),
    };
    base + u16::try_from(suffix_rows(suffix, entry_width(view, row, width, theme)))
        .unwrap_or(u16::MAX - base)
}

fn append_identity(lines: &mut Vec<Line<'static>>, suffix: &str, width: usize, theme: &Theme) {
    if suffix_rows(suffix, width) > 0 {
        // IDs and their brackets are validated ASCII; retain every byte across bounded rows.
        for part in suffix.trim_start().as_bytes().chunks(width.max(1)) {
            lines.push(Line::styled(
                std::str::from_utf8(part)
                    .expect("ASCII identity")
                    .to_owned(),
                theme.style(Role::TextSecondary),
            ));
        }
    }
}

pub(super) fn preferred_height(
    view: &ClientView,
    locale: Locale,
    width: u16,
    theme: &Theme,
) -> u16 {
    let rows = view.machines.sidebar_navigation_rows();
    let annotations = annotations(&rows, locale);
    sidebar_text::list_height(
        rows.iter()
            .zip(&annotations)
            .map(|(row, suffix)| entry_height(view, row, suffix, locale, width, theme)),
        view.machines.space_settings().row_gap(),
        2,
    )
}

pub(super) fn minimum_height(view: &ClientView, locale: Locale, width: u16, theme: &Theme) -> u16 {
    let rows = view.machines.sidebar_navigation_rows();
    let annotations = annotations(&rows, locale);
    rows.iter()
        .position(|row| view.machines.sidebar_focused_target().is_some_and(|target| row.target() == *target))
        .or_else(|| rows.iter().position(|row| matches!(row, SidebarRow::Context(row) if view.machines.current_workspace(row))))
        .or_else(|| (!rows.is_empty()).then_some(0))
        .map_or(1, |index| entry_height(view, &rows[index], &annotations[index], locale, width, theme))
        .saturating_add(2)
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
    let rows = view.machines.sidebar_navigation_rows();
    let title = format!(
        "{} {}",
        locale.shell().projects,
        rows.iter()
            .filter(|row| matches!(row, SidebarRow::Project { .. }))
            .count()
    );
    let header = Rect::new(area.x, area.y, area.width, 1);
    frame.render_widget(
        Paragraph::new(truncate_display(
            &title,
            usize::from(area.width),
            theme.glyphs.ellipsis,
        ))
        .style(theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD)),
        header,
    );
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, header, HitTarget::OpenMachineWorkspaces);
    }
    if area.height < 3 {
        return;
    }
    let body = Rect::new(area.x, area.y + 1, area.width, area.height - 2);
    let annotations = annotations(&rows, locale);
    let heights: Vec<_> = rows
        .iter()
        .zip(&annotations)
        .map(|(row, suffix)| entry_height(view, row, suffix, locale, area.width, theme))
        .collect();
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
        let target = row.target();
        let focused = view.machines.sidebar_focused_target() == Some(&target);
        let style = theme.style(if focused {
            Role::SurfaceSelection
        } else {
            Role::TextPrimary
        });
        frame.render_widget(Block::new().style(style), rect);
        let menu_width = menu_width(view, &target, theme).min(rect.width);
        let content = Rect::new(rect.x, rect.y, rect.width - menu_width, rect.height);
        let text_width = entry_width(view, row, rect.width, theme);
        let suffix = &annotations[index];
        let inline = if suffix_rows(suffix, text_width) == 0 {
            suffix.as_str()
        } else {
            ""
        };
        let (paint_area, mut row_lines) = match row {
            SidebarRow::Project { label, .. } => {
                let disclosure = Rect::new(content.x, content.y, content.width.min(3), 1);
                frame.render_widget(
                    Paragraph::new(if view.machines.sidebar_project_expanded(&target) {
                        "[-]"
                    } else {
                        "[+]"
                    })
                    .style(theme.style(Role::HintKey)),
                    disclosure,
                );
                if let Some(hits) = hits.as_deref_mut() {
                    hits.record(
                        layer,
                        disclosure,
                        HitTarget::SidebarDisclosure(target.clone()),
                    );
                }
                let text_area = Rect::new(
                    content.x + content.width.min(4),
                    content.y,
                    content.width.saturating_sub(4),
                    content.height,
                );
                let active = view
                    .machines
                    .current_context
                    .as_ref()
                    .is_some_and(|context| {
                        SidebarTarget::Context {
                            machine: view.machines.current.clone(),
                            target: context.clone(),
                        }
                        .parent()
                        .as_ref()
                            == Some(&target)
                    });
                let marker = if active {
                    theme.glyphs.active_marker
                } else {
                    " "
                };
                let machine = if view.machines.rows.iter().filter(|row| row.enabled).count() > 1 {
                    view.machines
                        .rows
                        .iter()
                        .find(|row| &row.choice == target.machine())
                        .map(|row| format!("{} / ", machine_label(view, row, locale)))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                let name = format!(
                    "{marker}{machine}{}",
                    if label.is_empty() {
                        locale.shell().projects
                    } else {
                        label
                    }
                );
                let line = format!(
                    "{}{inline}",
                    truncate_display(
                        &display_label(&name),
                        text_width.saturating_sub(display_width(inline)),
                        theme.glyphs.ellipsis
                    )
                );
                if let Some(hits) = hits.as_deref_mut() {
                    hits.record(
                        layer,
                        Rect::new(
                            content.x + disclosure.width,
                            content.y,
                            content.width.saturating_sub(disclosure.width),
                            content.height,
                        ),
                        HitTarget::MachineProjectRow {
                            target: target.clone(),
                            window,
                        },
                    );
                }
                (
                    text_area,
                    vec![Line::styled(
                        line,
                        theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
                    )],
                )
            }
            SidebarRow::Context(row) => {
                let indent = u16::from(target.parent().is_some()) * 2;
                let text_area = Rect::new(
                    content.x + indent.min(content.width),
                    content.y,
                    content.width.saturating_sub(indent),
                    content.height,
                );
                if let Some(hits) = hits.as_deref_mut() {
                    hits.record(
                        layer,
                        content,
                        HitTarget::MachineWorkspaceRow {
                            selected: row.selection(),
                            window,
                        },
                    );
                }
                (
                    text_area,
                    lines(view, row, locale, theme, text_width, inline),
                )
            }
        };
        append_identity(&mut row_lines, suffix, text_width, theme);
        frame.render_widget(
            Paragraph::new(
                row_lines
                    .into_iter()
                    .map(|line| fit(line, text_width, theme.glyphs.ellipsis))
                    .collect::<Vec<_>>(),
            )
            .style(style),
            paint_area,
        );
        if menu_width > 0 {
            let menu = Rect::new(content.right(), rect.y, menu_width, 1);
            frame.render_widget(
                Paragraph::new(format!("[{}]", theme.glyphs.ellipsis))
                    .style(theme.style(Role::HintKey)),
                menu,
            );
            if let Some(hits) = hits.as_deref_mut() {
                hits.record(layer, menu, HitTarget::SidebarMenu(target));
            }
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
