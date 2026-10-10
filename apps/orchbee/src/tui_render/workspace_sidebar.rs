//! Workspace token rows over complete canonical checkpoints and attention-projected Agent states.
use super::*;
use crate::{
    machine_discovery::display_label,
    machine_picker::{Row, SidebarRow, SidebarTarget},
};
use orchbee_local_site_runtime::{WindowsSidebarTokenStyle, WindowsSpaceSidebarTokenKind as Kind};
use ratatui::style::Style;
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
    let text_style = |role| {
        let style = theme.style(role);
        if current {
            theme.current_context_style(style)
        } else {
            style
        }
    };
    let machine = if view.machines.rows.iter().filter(|m| m.enabled).count() > 1 {
        format!(
            "{} / ",
            truncate_display(&machine_label(view, row, locale), 8, theme.glyphs.ellipsis)
        )
    } else {
        String::new()
    };
    let mut lines = Vec::new();
    let mut identity = machine.clone();
    let continuation_indent = display_width(&machine) + 2;
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
        if lines.is_empty() {
            identity = format!(
                "{machine}{}",
                if resolved
                    .first()
                    .is_some_and(|(kind, _, _)| *kind == Kind::StateIcon)
                {
                    ""
                } else {
                    "  "
                }
            );
        }
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
                continuation_indent
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
                    text_style(Role::TextSecondary),
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
                        theme.sidebar_token_style(Role::StatusSuccess, style, current),
                    ));
                }
                if git.ahead > 0 && git.behind > 0 {
                    spans.push(Span::raw(" "));
                }
                if git.behind > 0 {
                    spans.push(Span::styled(
                        format!("{}{n}", theme.glyphs.git_behind, n = git.behind),
                        theme.sidebar_token_style(Role::StatusError, style, current),
                    ));
                }
            } else {
                let role = match kind {
                    Kind::StateIcon | Kind::StateText => context.role,
                    Kind::Branch => Role::TextSecondary,
                    _ if !lines.is_empty() => Role::TextSecondary,
                    _ => Role::TextPrimary,
                };
                let mut projected = theme.sidebar_token_style(role, style, current);
                if kind == Kind::Workspace && current && style.bold.is_none() {
                    projected = projected.add_modifier(Modifier::BOLD);
                }
                spans.push(Span::styled(text, projected));
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
            if !lines.is_empty() {
                spans.insert(0, Span::raw(" ".repeat(continuation_indent)));
            }
            lines.push(Line::from(spans));
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
    }
    lines[0]
        .spans
        .insert(0, Span::styled(identity, text_style(Role::TextSecondary)));
    lines[0] = fit(
        std::mem::take(&mut lines[0]),
        width.saturating_sub(display_width(suffix)),
        theme.glyphs.ellipsis,
    );
    lines[0].spans.push(Span::styled(
        suffix.to_owned(),
        text_style(Role::TextSecondary),
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
        SidebarTarget::Agent { target, .. } => target.member.as_str(),
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
        SidebarRow::Context { row, .. } => {
            row.workspace_label().unwrap_or(locale.shell().standalone)
        }
        SidebarRow::Agent { row, .. } => row
            .sidebar_agent()
            .map_or("", crate::machine_agents::Row::title),
    })
}

fn annotations(view: &ClientView, rows: &[SidebarRow<'_>], locale: Locale) -> Vec<String> {
    let mut groups = std::collections::BTreeMap::new();
    for (index, row) in rows.iter().enumerate() {
        let target = row.target();
        let machine = match target.machine() {
            crate::machine_picker::Choice::Local => None,
            crate::machine_picker::Choice::Remote(id) => Some(id.as_str().to_owned()),
        };
        let parent = view
            .machines
            .sidebar_parent(&target)
            .map(|parent| identity(&parent).to_owned());
        groups
            .entry((
                machine,
                target.site_id().as_str().to_owned(),
                parent,
                match row {
                    SidebarRow::Project { .. } => 0,
                    SidebarRow::Context { .. } => 1,
                    SidebarRow::Agent { .. } => 2,
                },
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
    if matches!(
        target,
        SidebarTarget::Project { .. } | SidebarTarget::Context { .. }
    ) && target.machine() == &view.machines.current
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

fn create_width(view: &ClientView, target: &SidebarTarget, theme: &Theme) -> u16 {
    if matches!(target, SidebarTarget::Project { .. }) && menu_width(view, target, theme) > 0 {
        3
    } else {
        0
    }
}

fn entry_width(view: &ClientView, row: &SidebarRow<'_>, width: u16, theme: &Theme) -> usize {
    let indent = match row {
        SidebarRow::Project { .. } => 4,
        SidebarRow::Context { row, has_agents } => {
            2 + u16::from(
                row.sidebar_target()
                    .and_then(|target| view.machines.sidebar_parent(&target))
                    .is_some(),
            ) * 2
                + u16::from(*has_agents) * 2
        }
        SidebarRow::Agent { row, .. } => {
            4 + u16::from(
                row.sidebar_agent_target()
                    .and_then(|target| view.machines.sidebar_parent(&target))
                    .and_then(|target| view.machines.sidebar_parent(&target))
                    .is_some(),
            ) * 2
        }
    };
    usize::from(width.saturating_sub(
        menu_width(view, &row.target(), theme) + create_width(view, &row.target(), theme) + indent,
    ))
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
        SidebarRow::Context { row, .. } => height(view, row, locale),
        SidebarRow::Agent { row, .. } => super::agent_sidebar::tokens::height(view, row, locale),
    };
    let text_width = entry_width(view, row, width, theme);
    let base = base + summary_lines(view, row, locale, theme, text_width).len() as u16;
    base + u16::try_from(suffix_rows(suffix, text_width)).unwrap_or(u16::MAX - base)
}

fn summary_lines(
    view: &ClientView,
    row: &SidebarRow<'_>,
    locale: Locale,
    theme: &Theme,
    width: usize,
) -> Vec<Line<'static>> {
    let target = row.target();
    let collapsed = match row {
        SidebarRow::Project { .. } => !view.machines.sidebar_project_expanded(&target),
        SidebarRow::Context { .. } => !view.machines.sidebar_context_agents_expanded(&target),
        SidebarRow::Agent { .. } => false,
    };
    let Some(counts) = collapsed
        .then(|| view.machines.sidebar_agent_counts(&target))
        .flatten()
    else {
        return Vec::new();
    };
    if counts.total == 0 {
        return Vec::new();
    }
    let count = locale.sidebar_agent_count(counts.total);
    let mut lines = vec![Line::styled(
        count.clone(),
        theme.style(Role::TextSecondary),
    )];
    if counts.waiting > 0 {
        let waiting = locale.sidebar_waiting_count(counts.waiting);
        let span = Span::styled(waiting.clone(), theme.style(Role::StatusWarning));
        if display_width(&count) + 3 + display_width(&waiting) <= width {
            lines[0]
                .spans
                .push(Span::styled(" · ", theme.style(Role::TextSecondary)));
            lines[0].spans.push(span);
        } else {
            lines.push(Line::from(span));
        }
    }
    lines
}

fn append_identity(lines: &mut Vec<Line<'static>>, suffix: &str, width: usize, style: Style) {
    if suffix_rows(suffix, width) > 0 {
        // IDs and their brackets are validated ASCII; retain every byte across bounded rows.
        for part in suffix.trim_start().as_bytes().chunks(width.max(1)) {
            lines.push(Line::styled(
                std::str::from_utf8(part)
                    .expect("ASCII identity")
                    .to_owned(),
                style,
            ));
        }
    }
}

fn minimum_height(
    view: &ClientView,
    rows: &[SidebarRow<'_>],
    locale: Locale,
    width: u16,
    theme: &Theme,
) -> u16 {
    let annotations = annotations(view, rows, locale);
    rows.iter()
        .position(|row| {
            view.machines
                .sidebar_focused_target()
                .is_some_and(|target| row.target() == *target)
        })
        .or_else(|| {
            rows.iter().position(|row| {
                matches!(row, SidebarRow::Agent { row, .. } if row.sidebar_agent().is_some_and(|agent| view.machines.current_agent(&row.choice, agent)))
            })
        })
        .or_else(|| {
            rows.iter().position(|row| {
                matches!(row, SidebarRow::Context { row, .. } if view.machines.current_workspace(row))
            })
        })
        .or_else(|| (!rows.is_empty()).then_some(0))
        .map_or(1, |index| {
            entry_height(
                view,
                &rows[index],
                &annotations[index],
                locale,
                width,
                theme,
            )
        })
        .saturating_add(2)
}

pub(super) struct RenderContext<'a> {
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) layer: HitLayer,
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    context: RenderContext<'_>,
    rows: &[SidebarRow<'_>],
    mut hits: Option<&mut HitMapBuilder>,
) {
    let RenderContext {
        view,
        locale,
        theme,
        layer,
    } = context;
    if area.is_empty() {
        return;
    }
    let count = view.machines.sidebar_workspace_count();
    let create = Rect::new(
        area.right() - area.width.min(3),
        area.y,
        area.width.min(3),
        1,
    );
    let filter_label = locale.machine_picker_text("agent-sidebar-open");
    let filter_width = if area.width >= 18 {
        u16::try_from(display_width(filter_label) + 1)
            .unwrap_or(u16::MAX)
            .min(9)
            .min(create.x.saturating_sub(area.x + 10))
    } else {
        0
    };
    let filter = Rect::new(create.x - filter_width, area.y, filter_width, 1);
    let header = Rect::new(area.x, area.y, filter.x.saturating_sub(area.x), 1);
    let header_count = if count > 1 {
        count.to_string()
    } else {
        String::new()
    };
    let browse = locale.machine_picker_text("workspace-sidebar-open");
    let browse = if display_width(locale.shell().workspaces)
        + display_width(&header_count)
        + usize::from(!header_count.is_empty())
        + display_width(browse)
        + 2
        <= usize::from(header.width)
    {
        browse
    } else {
        ""
    };
    sidebar_text::section_heading(
        frame,
        header,
        locale.shell().workspaces,
        &header_count,
        browse,
        theme,
    );
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, header, HitTarget::OpenMachineWorkspaces);
    }
    if filter_width > 0 {
        frame.render_widget(
            Paragraph::new(truncate_display(
                filter_label,
                usize::from(filter.width.saturating_sub(1)),
                theme.glyphs.ellipsis,
            ))
            .alignment(Alignment::Right)
            .style(theme.style(Role::HintLabel)),
            filter,
        );
        if let Some(hits) = hits.as_deref_mut() {
            hits.record(layer, filter, HitTarget::OpenMachineAgents);
        }
    }
    frame.render_widget(
        Paragraph::new(" + ").style(theme.style(Role::HintLabel)),
        create,
    );
    if let Some(hits) = hits.as_deref_mut() {
        hits.record(layer, create, HitTarget::CreateMenu(None));
    }
    if area.height < 3 {
        return;
    }
    let header_rows = if area.height > minimum_height(view, rows, locale, area.width, theme) {
        2
    } else {
        1
    };
    let body = Rect::new(
        area.x,
        area.y + header_rows,
        area.width,
        area.height - header_rows - 1,
    );
    let annotations = annotations(view, rows, locale);
    let heights: Vec<_> = rows
        .iter()
        .zip(&annotations)
        .map(|(row, suffix)| entry_height(view, row, suffix, locale, area.width, theme))
        .collect();
    let space_gap = view.machines.space_settings().row_gap();
    let agent_gap = view.machines.sidebar_settings().row_gap();
    // A gap belongs to the row that follows it. This preserves Agent-configured
    // spacing before/between nested Agent rows while the whole tree shares one window.
    let gaps: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(index, _)| {
            if matches!(rows.get(index + 1), Some(SidebarRow::Agent { .. })) {
                agent_gap
            } else {
                space_gap
            }
        })
        .collect();
    let window = view
        .machines
        .space_window_for_row_gaps(rows, &heights, &gaps, body.height);
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
        let current = match row {
            SidebarRow::Context { row, .. } => view.machines.current_workspace(row),
            SidebarRow::Agent { row, .. } => row
                .sidebar_agent()
                .is_some_and(|agent| view.machines.current_agent(&row.choice, agent)),
            SidebarRow::Project { .. } => {
                !view.machines.sidebar_project_expanded(&target)
                    && view
                        .machines
                        .current_context
                        .as_ref()
                        .is_some_and(|context| {
                            view.machines
                                .sidebar_parent(&SidebarTarget::Context {
                                    machine: view.machines.current.clone(),
                                    target: context.clone(),
                                })
                                .as_ref()
                                == Some(&target)
                        })
            }
        };
        let style = if current {
            theme.current_context_style(theme.style(Role::TextPrimary))
        } else {
            theme.style(Role::TextPrimary)
        };
        frame.render_widget(Block::new().style(style), rect);
        let menu_width = menu_width(view, &target, theme).min(rect.width);
        let create_width = create_width(view, &target, theme).min(rect.width - menu_width);
        let content = Rect::new(
            rect.x,
            rect.y,
            rect.width - menu_width - create_width,
            rect.height,
        );
        let text_width = entry_width(view, row, rect.width, theme);
        let suffix = &annotations[index];
        let inline = if suffix_rows(suffix, text_width) == 0 {
            suffix.as_str()
        } else {
            ""
        };
        let (paint_area, mut row_lines) = match row {
            SidebarRow::Project { label, .. } => {
                let gutter = content.width.min(2);
                let disclosure = Rect::new(
                    content.x + gutter,
                    content.y,
                    content.width.saturating_sub(gutter).min(2),
                    1,
                );
                frame.render_widget(
                    Paragraph::new(if view.machines.sidebar_project_expanded(&target) {
                        theme.glyphs.disclosure_open
                    } else {
                        theme.glyphs.disclosure_closed
                    })
                    .style(if current {
                        style
                    } else {
                        theme.style(Role::TextSecondary)
                    }),
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
                    "{machine}{}",
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
                        Rect::new(content.x, content.y, gutter, content.height),
                        HitTarget::MachineProjectRow {
                            target: target.clone(),
                            window,
                        },
                    );
                    if content.height > 1 {
                        hits.record(
                            layer,
                            Rect::new(
                                disclosure.x,
                                content.y + 1,
                                disclosure.width,
                                content.height - 1,
                            ),
                            HitTarget::MachineProjectRow {
                                target: target.clone(),
                                window,
                            },
                        );
                    }
                    hits.record(
                        layer,
                        Rect::new(
                            disclosure.right(),
                            content.y,
                            content.width.saturating_sub(gutter + disclosure.width),
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
                        if current {
                            style.add_modifier(Modifier::BOLD)
                        } else {
                            theme.style(Role::TextSecondary)
                        },
                    )],
                )
            }
            SidebarRow::Context { row, has_agents } => {
                let indent = 2 + u16::from(view.machines.sidebar_parent(&target).is_some()) * 2;
                let disclosure_width = u16::from(*has_agents) * 2;
                let disclosure = Rect::new(
                    content.x + indent.min(content.width),
                    content.y,
                    content.width.saturating_sub(indent).min(disclosure_width),
                    1,
                );
                if *has_agents {
                    frame.render_widget(
                        Paragraph::new(if view.machines.sidebar_context_agents_expanded(&target) {
                            theme.glyphs.disclosure_open
                        } else {
                            theme.glyphs.disclosure_closed
                        })
                        .style(if current {
                            style
                        } else {
                            theme.style(Role::TextSecondary)
                        }),
                        disclosure,
                    );
                }
                let text_area = Rect::new(
                    disclosure.right(),
                    content.y,
                    content
                        .width
                        .saturating_sub(indent.saturating_add(disclosure_width)),
                    content.height,
                );
                if let Some(hits) = hits.as_deref_mut() {
                    let selected = row.selection();
                    if *has_agents {
                        hits.record(
                            layer,
                            Rect::new(
                                content.x,
                                content.y,
                                disclosure.x.saturating_sub(content.x),
                                content.height,
                            ),
                            HitTarget::MachineWorkspaceRow {
                                selected: selected.clone(),
                                window,
                            },
                        );
                        hits.record(
                            layer,
                            disclosure,
                            HitTarget::SidebarDisclosure(target.clone()),
                        );
                        if content.height > 1 {
                            hits.record(
                                layer,
                                Rect::new(
                                    disclosure.x,
                                    disclosure.y.saturating_add(1),
                                    disclosure.width,
                                    content.height - 1,
                                ),
                                HitTarget::MachineWorkspaceRow {
                                    selected: selected.clone(),
                                    window,
                                },
                            );
                        }
                        hits.record(
                            layer,
                            Rect::new(
                                disclosure.right(),
                                content.y,
                                content.right().saturating_sub(disclosure.right()),
                                content.height,
                            ),
                            HitTarget::MachineWorkspaceRow { selected, window },
                        );
                    } else {
                        hits.record(
                            layer,
                            content,
                            HitTarget::MachineWorkspaceRow { selected, window },
                        );
                    }
                }
                (
                    text_area,
                    lines(view, row, locale, theme, text_width, inline),
                )
            }
            SidebarRow::Agent { row, ordinal } => {
                let indent = 4 + u16::from(
                    view.machines
                        .sidebar_parent(&target)
                        .and_then(|target| view.machines.sidebar_parent(&target))
                        .is_some(),
                ) * 2;
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
                        HitTarget::MachineAgentRow {
                            selected: row.selection(),
                            window,
                        },
                    );
                }
                (
                    text_area,
                    super::agent_sidebar::tokens::lines(view, row, *ordinal, locale, theme),
                )
            }
        };
        let identity_style = if current {
            theme.current_context_style(theme.style(Role::TextSecondary))
        } else {
            theme.style(Role::TextSecondary)
        };
        if matches!(row, SidebarRow::Agent { .. }) && !inline.is_empty() {
            if row_lines.is_empty() {
                row_lines.push(Line::default());
            }
            row_lines[0] = fit(
                std::mem::take(&mut row_lines[0]),
                text_width.saturating_sub(display_width(inline)),
                theme.glyphs.ellipsis,
            );
            row_lines[0]
                .spans
                .push(Span::styled(inline.to_owned(), identity_style));
        }
        row_lines.extend(
            summary_lines(view, row, locale, theme, text_width)
                .into_iter()
                .map(|mut line| {
                    if current {
                        for span in &mut line.spans {
                            span.style = theme.current_context_style(span.style);
                        }
                    }
                    line
                }),
        );
        append_identity(&mut row_lines, suffix, text_width, identity_style);
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
        if focused && !content.is_empty() {
            frame.render_widget(
                Paragraph::new(">").style(if current {
                    style.add_modifier(Modifier::BOLD)
                } else {
                    theme.style(Role::AccentFocus).add_modifier(Modifier::BOLD)
                }),
                Rect::new(content.x, content.y, 1, 1),
            );
        }
        if current && content.width > 1 && !matches!(row, SidebarRow::Agent { .. }) {
            frame.render_widget(
                Paragraph::new(theme.current_context_marker()).style(style),
                Rect::new(content.x + 1, content.y, 1, 1),
            );
        }
        if rect.height > 1
            && menu_width + create_width > 0
            && let Some(hits) = hits.as_deref_mut()
        {
            let row_target = match row {
                SidebarRow::Project { .. } => HitTarget::MachineProjectRow {
                    target: target.clone(),
                    window,
                },
                SidebarRow::Context { row, .. } => HitTarget::MachineWorkspaceRow {
                    selected: row.selection(),
                    window,
                },
                SidebarRow::Agent { .. } => {
                    unreachable!("Agent rows have no menu or creation control")
                }
            };
            hits.record(
                layer,
                Rect::new(
                    content.right(),
                    rect.y + 1,
                    menu_width + create_width,
                    rect.height - 1,
                ),
                row_target,
            );
        }
        if create_width > 0 {
            let create = Rect::new(content.right(), rect.y, create_width, 1);
            frame.render_widget(
                Paragraph::new(" + ").style(if current {
                    theme.current_context_style(theme.style(Role::HintLabel))
                } else {
                    theme.style(Role::HintLabel)
                }),
                create,
            );
            if let Some(hits) = hits.as_deref_mut() {
                hits.record(layer, create, HitTarget::CreateMenu(Some(target.clone())));
            }
        }
        if menu_width > 0 {
            let menu = Rect::new(content.right() + create_width, rect.y, menu_width, 1);
            frame.render_widget(
                Paragraph::new(format!(" {} ", theme.glyphs.ellipsis)).style(if current {
                    theme.current_context_style(theme.style(Role::HintLabel))
                } else {
                    theme.style(Role::HintLabel)
                }),
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
                gaps[index].min(body.bottom().saturating_sub(y)),
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
    let range = if window.top > 0 || window.end() < window.total {
        format!(
            "{}-{}/{}",
            if window.capacity > 0 {
                window.top + 1
            } else {
                0
            },
            window.end(),
            window.total
        )
    } else {
        String::new()
    };
    let count_width = display_width(&range).min(usize::from(footer.width)) as u16;
    let has_range = !range.is_empty();
    frame.render_widget(
        Paragraph::new(range)
            .alignment(Alignment::Right)
            .style(theme.style(Role::TextSecondary)),
        Rect::new(footer.right() - count_width, footer.y, count_width, 1),
    );
    if has_range && let Some(hits) = hits {
        hits.record(layer, footer, HitTarget::OpenMachineWorkspaces);
    }
}
#[cfg(test)]
mod tests;
