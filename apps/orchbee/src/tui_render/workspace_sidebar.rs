//! Workspace token rows over complete canonical checkpoints and attention-projected Agent states.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, RowContentBand,
    RowFocusCue, RowTracks, action_rect, paint_pointer_target, paint_row_interaction,
    render_action,
};
use super::{
    Block, ClientView, Frame, HitLayer, HitMapBuilder, Line, Locale, Modifier, Paragraph, Rect,
    Role, Span, Theme, display_width, sidebar_text, truncate_display,
};
use crate::{
    machine_discovery::display_label,
    machine_picker::{
        Row, SidebarFixedAction, SidebarPart, SidebarRow, SidebarSource, SidebarTarget,
    },
    tui_pointer::{InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer},
};
use orchbee_local_site_runtime::{WindowsSidebarTokenStyle, WindowsSpaceSidebarTokenKind as Kind};
use ratatui::style::Style;
use sidebar_text::{fit, machine_label, state_role};
mod layout;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::tui_render) enum CurrentIdentityEmphasis {
    Theme,
    ExplicitPlain,
}

pub(in crate::tui_render) struct ProjectedSidebarLines {
    lines: Vec<Line<'static>>,
    current_identity: CurrentIdentityEmphasis,
}

impl ProjectedSidebarLines {
    pub(in crate::tui_render) fn new(
        lines: Vec<Line<'static>>,
        current_identity: CurrentIdentityEmphasis,
    ) -> Self {
        Self {
            lines,
            current_identity,
        }
    }

    pub(in crate::tui_render) fn into_parts(self) -> (Vec<Line<'static>>, CurrentIdentityEmphasis) {
        (self.lines, self.current_identity)
    }
}

struct Context<'a> {
    row: &'a Row,
    observed: Option<&'a crate::machine_spaces::Row>,
    state: Option<&'static str>,
    glyph_state: Option<orchbee_workbench_protocol::MetadataState>,
    role: Role,
    standalone: &'static str,
}
impl<'a> Context<'a> {
    fn new(view: &'a ClientView, row: &'a Row, locale: Locale) -> Self {
        let glyph_state = view.machines.workspace_state(row);
        let (key, role) =
            state_role(glyph_state.unwrap_or(orchbee_workbench_protocol::MetadataState::Unknown));
        Self {
            row,
            observed: view.machines.space_checkpoint_row(row),
            state: glyph_state.map(|_| locale.machine_picker_text(key)),
            glyph_state,
            role,
            standalone: locale.shell().standalone,
        }
    }
    fn value(&self, kind: Kind<'_>) -> Option<&str> {
        match kind {
            Kind::StateIcon | Kind::GitStatus => None,
            Kind::StateText => self.state,
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
) -> ProjectedSidebarLines {
    let context = Context::new(view, row, locale);
    let text_style = |role| theme.workspace_token_style(role, Default::default(), false, false);
    let machine = if view.machines.rows.iter().filter(|m| m.enabled).count() > 1 {
        format!(
            "{} / ",
            truncate_display(&machine_label(view, row, locale), 8, theme.glyphs.ellipsis)
        )
    } else {
        String::new()
    };
    let mut lines = Vec::new();
    let mut current_identity = CurrentIdentityEmphasis::Theme;
    let identity = machine.clone();
    for tokens in view.machines.space_settings().rows() {
        let resolved: Vec<_> = tokens
            .iter()
            .filter_map(|token| {
                let kind = token.kind();
                if kind == Kind::StateIcon {
                    return None;
                }
                let style = token.style_for_value(context.value(kind));
                if !context.visible(kind, style) {
                    return None;
                }
                let text = match kind {
                    Kind::StateIcon => context
                        .glyph_state
                        .map_or(" ", |state| sidebar_text::state_glyph(state, theme))
                        .to_owned(),
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
            let identity_token =
                kind == Kind::Workspace || lines.is_empty() && matches!(kind, Kind::Custom(_));
            if identity_token && style.bold == Some(false) {
                current_identity = CurrentIdentityEmphasis::ExplicitPlain;
            }
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
                    if previous_icon {
                        " ".into()
                    } else {
                        format!(" {} ", theme.glyphs.middle_dot)
                    },
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
                        theme.workspace_token_style(Role::StatusSuccess, style, false, false),
                    ));
                }
                if git.ahead > 0 && git.behind > 0 {
                    spans.push(Span::raw(" "));
                }
                if git.behind > 0 {
                    spans.push(Span::styled(
                        format!("{}{n}", theme.glyphs.git_behind, n = git.behind),
                        theme.workspace_token_style(Role::StatusError, style, false, false),
                    ));
                }
            } else {
                let role = match kind {
                    Kind::StateIcon | Kind::StateText => context.role,
                    Kind::Branch => Role::TextSecondary,
                    _ if !lines.is_empty() => Role::TextSecondary,
                    _ => Role::TextPrimary,
                };
                spans.push(Span::styled(
                    text,
                    theme.workspace_token_style(role, style, false, false),
                ));
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
    ProjectedSidebarLines::new(lines, current_identity)
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

fn depth(view: &ClientView, target: &SidebarTarget) -> u16 {
    let mut depth = 0;
    let mut parent = view.machines.sidebar_parent(target);
    while let Some(target) = parent {
        depth += 1;
        parent = view.machines.sidebar_parent(&target);
    }
    depth
}

fn auxiliary_actions(source: &SidebarSource, target: &SidebarTarget) -> layout::AuxiliaryActions {
    layout::AuxiliaryActions::new(
        source.has_part(target, SidebarPart::Menu),
        source.has_part(target, SidebarPart::Create),
    )
}

fn entry_width(
    view: &ClientView,
    source: &SidebarSource,
    row: &SidebarRow<'_>,
    width: u16,
    sidebar_width: u16,
) -> usize {
    let target = row.target();
    layout::Tracks::resolve_for_width(
        Rect::new(0, 0, width, 1),
        0,
        depth(view, &target),
        sidebar_width,
        auxiliary_actions(source, &target),
        layout::RowKind::for_target(&target),
    )
    .map_or(1, |tracks| usize::from(tracks.primary.width))
}

fn suffix_rows(suffix: &str, width: usize) -> usize {
    if display_width(suffix) <= width.saturating_sub(4) {
        0
    } else {
        suffix.trim_start().len().div_ceil(width.max(1))
    }
}

#[derive(Clone, Copy)]
struct EntryMeasureContext<'a> {
    view: &'a ClientView,
    source: &'a SidebarSource,
    locale: Locale,
    sidebar_width: u16,
    theme: &'a Theme,
}

fn entry_height(
    context: EntryMeasureContext<'_>,
    row: &SidebarRow<'_>,
    suffix: &str,
    width: u16,
) -> u16 {
    let EntryMeasureContext {
        view,
        source,
        locale,
        sidebar_width,
        theme,
    } = context;
    let base = match row {
        SidebarRow::Project { .. } => 1,
        SidebarRow::Context { row, .. } => height(view, row, locale),
        SidebarRow::Agent { row, .. } => super::agent_sidebar::tokens::height(view, row, locale),
    };
    let text_width = entry_width(view, source, row, width, sidebar_width);
    let base = base + summary_lines(view, row, locale, theme, text_width).len() as u16;
    base + u16::try_from(suffix_rows(suffix, text_width)).unwrap_or(u16::MAX - base)
}

fn continuation_style(_view: &ClientView, row: &SidebarRow<'_>, theme: &Theme) -> Style {
    if let SidebarRow::Context { .. } = row {
        theme.workspace_token_style(Role::TextSecondary, Default::default(), false, false)
    } else {
        theme.style(Role::TextSecondary)
    }
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
    let secondary = continuation_style(view, row, theme);
    let mut lines = vec![Line::styled(count.clone(), secondary)];
    if counts.waiting > 0 {
        let waiting = locale.sidebar_waiting_count(counts.waiting);
        let span = Span::styled(waiting.clone(), theme.style(Role::StatusWarning));
        if display_width(&count) + 3 + display_width(&waiting) <= width {
            lines[0].spans.push(Span::styled(" · ", secondary));
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
    source: &SidebarSource,
    rows: &[SidebarRow<'_>],
    locale: Locale,
    width: u16,
    sidebar_width: u16,
    theme: &Theme,
) -> u16 {
    let annotations = annotations(view, rows, locale);
    let measure = EntryMeasureContext {
        view,
        source,
        locale,
        sidebar_width,
        theme,
    };
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
            entry_height(measure, &rows[index], &annotations[index], width)
        })
        .saturating_add(2)
}

pub(super) struct RenderContext<'a> {
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) layer: HitLayer,
    /// The complete painted Sidebar surface, before frame and padding are removed.
    pub(super) sidebar_width: u16,
    pub(super) owns_keyboard: bool,
    pub(super) source: &'a SidebarSource,
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

fn interaction_spec(contract: PrimaryContract, enabled: bool) -> InteractionSpec {
    if enabled {
        InteractionSpec::enabled(contract)
    } else {
        InteractionSpec::disabled(contract)
    }
}

fn winning_surface(pointer: PointerPresentation, focused: bool, current: bool) -> Option<Role> {
    match pointer {
        PointerPresentation::Pressed => Some(Role::SurfacePressed),
        PointerPresentation::Rest | PointerPresentation::Hovered if focused => {
            Some(Role::SurfaceSelection)
        }
        PointerPresentation::Hovered => Some(Role::SurfaceHover),
        PointerPresentation::Rest if current => Some(Role::SurfaceCurrent),
        PointerPresentation::Rest => None,
    }
}

fn semantic_role(style: Style, theme: &Theme) -> Option<Role> {
    [
        Role::TextPrimary,
        Role::TextSecondary,
        Role::TextDisabled,
        Role::HintKey,
        Role::HintLabel,
        Role::StatusSuccess,
        Role::StatusWarning,
        Role::StatusError,
        Role::StatusInfo,
        Role::StatusPending,
        Role::StatusWorking,
        Role::StatusUnread,
    ]
    .into_iter()
    .find(|role| style.fg == theme.style(*role).fg)
}

fn compose_lines(
    lines: &mut [Line<'static>],
    pointer: PointerPresentation,
    focused: bool,
    current: bool,
    theme: &Theme,
) {
    let interaction = winning_surface(pointer, focused, current);
    for (index, line) in lines.iter_mut().enumerate() {
        let line_role = semantic_role(line.style, theme);
        line.style =
            theme.content_on_surface(Role::SurfaceChrome, interaction, line_role, line.style);
        for span in &mut line.spans {
            let explicitly_plain = span.style.sub_modifier.contains(Modifier::BOLD);
            let role = semantic_role(span.style, theme);
            span.style =
                theme.content_on_surface(Role::SurfaceChrome, interaction, role, span.style);
            if explicitly_plain {
                span.style = span.style.remove_modifier(Modifier::BOLD);
            } else if current && index == 0 {
                span.style = span.style.add_modifier(Modifier::BOLD);
            }
        }
    }
}

fn restore_interaction_background(
    frame: &mut Frame<'_>,
    rect: Rect,
    interaction: Option<Role>,
    theme: &Theme,
) {
    let Some(background) = interaction.and_then(|role| theme.style(role).bg) else {
        return;
    };
    // Ratatui clears a wide glyph's hidden continuation cells while painting its Paragraph.
    // Restore the row surface after content, then let an active child pointer surface win below.
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            frame.buffer_mut()[(x, y)].set_bg(background);
        }
    }
}

fn row_state(
    sidebar_focused: bool,
    focused: bool,
    current: bool,
    pointer: PointerPresentation,
) -> InteractionState {
    InteractionState::new(
        if sidebar_focused {
            FocusState::Focused
        } else {
            FocusState::Unfocused
        },
        pointer,
        if focused {
            NavigationState::Provisional
        } else {
            NavigationState::None
        },
        if current {
            CurrentState::Current
        } else {
            CurrentState::NotCurrent
        },
        CheckedState::Unchecked,
        Availability::Enabled,
        OperationState::Idle,
    )
}

struct ChildInteraction<'a> {
    layer: HitLayer,
    rect: Rect,
    target: &'a InteractionTarget,
    spec: InteractionSpec,
    role: Role,
}

fn prepare_child(
    frame: &mut Frame<'_>,
    hits: &mut HitMapBuilder,
    child: ChildInteraction<'_>,
    theme: &Theme,
) -> PointerPresentation {
    let ChildInteraction {
        layer,
        rect,
        target,
        spec,
        role,
    } = child;
    let pointer = pointer_presentation(hits.projected_pointer(layer, rect, rect, target, spec));
    if paint_pointer_target(
        frame,
        rect,
        ControlSurface::Chrome,
        pointer,
        role,
        Availability::Enabled,
        theme,
    )
    .is_some()
    {
        hits.record_interaction(layer, rect, rect, target.clone(), spec);
    }
    pointer
}

fn padded_action(label: &str, shortcut: Option<&str>, width: usize) -> String {
    let action = format!("[ {label} ]");
    let full = shortcut.map_or_else(|| action.clone(), |shortcut| format!("{action} {shortcut}"));
    let mut result = if display_width(&full) <= width {
        full
    } else {
        action
    };
    result.push_str(&" ".repeat(width.saturating_sub(display_width(&result))));
    result
}

fn current_row(view: &ClientView, row: &SidebarRow<'_>, target: &SidebarTarget) -> bool {
    match row {
        SidebarRow::Context { row, .. } => view.machines.current_workspace(row),
        SidebarRow::Agent { row, .. } => row
            .sidebar_agent()
            .is_some_and(|agent| view.machines.current_agent(&row.choice, agent)),
        SidebarRow::Project { .. } => {
            !view.machines.sidebar_project_expanded(target)
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
                            == Some(target)
                    })
        }
    }
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
        sidebar_width,
        owns_keyboard,
        source,
    } = context;
    if area.is_empty() {
        return;
    }
    let count = view.machines.sidebar_workspace_count();
    let header = Rect::new(area.x, area.y, area.width, 1);
    sidebar_text::section_heading(
        frame,
        header,
        locale.shell().workspaces,
        &count.to_string(),
        "",
        theme,
    );
    if area.height < 3 {
        return;
    }
    let header_rows = if area.height >= 10
        && area.height
            > minimum_height(view, source, rows, locale, area.width, sidebar_width, theme)
    {
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
    let measure = EntryMeasureContext {
        view,
        source,
        locale,
        sidebar_width,
        theme,
    };
    let heights: Vec<_> = rows
        .iter()
        .zip(&annotations)
        .map(|(row, suffix)| entry_height(measure, row, suffix, area.width))
        .collect();
    let space_gap = view.machines.space_settings().row_gap();
    let agent_gap = view.machines.sidebar_settings().row_gap();
    let gaps: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(index, _)| match rows.get(index + 1) {
            Some(SidebarRow::Agent { .. }) => agent_gap,
            Some(SidebarRow::Project { .. }) => space_gap.max(1),
            _ => space_gap,
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
        let leading_gap = if index == window.top {
            0
        } else {
            gaps[index - 1].min(body.bottom().saturating_sub(y))
        };
        let content_height =
            heights[index].min(body.bottom().saturating_sub(y).saturating_sub(leading_gap));
        if content_height == 0 {
            break;
        }
        let band = Rect::new(
            body.x,
            y,
            body.width,
            leading_gap.saturating_add(content_height),
        );
        let target = row.target();
        let actions = auxiliary_actions(source, &target);
        let Some(tracks) = layout::Tracks::resolve_for_width(
            band,
            leading_gap,
            depth(view, &target),
            sidebar_width,
            actions,
            layout::RowKind::for_target(&target),
        ) else {
            break;
        };
        let focused = owns_keyboard && view.machines.sidebar_focused_target() == Some(&target);
        let current = current_row(view, row, &target);
        frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), band);

        let disclosure_drawn = source.drawn(target.clone(), SidebarPart::Disclosure, window);
        let menu_drawn = source.drawn(target.clone(), SidebarPart::Menu, window);
        let create_drawn = tracks
            .density()
            .direct_create()
            .then(|| source.drawn(target.clone(), SidebarPart::Create, window))
            .flatten();

        let disclosure_target = disclosure_drawn.map(InteractionTarget::Sidebar);
        let menu_target = menu_drawn.map(InteractionTarget::Sidebar);
        let create_target = create_drawn.map(InteractionTarget::Sidebar);
        let disclosure_spec = interaction_spec(PrimaryContract::ActivateOnRelease, true);
        let auxiliary_spec = interaction_spec(PrimaryContract::Specialized, true);

        let disclosure_pointer =
            if let (Some(hits), Some(target)) = (hits.as_deref_mut(), disclosure_target.as_ref()) {
                prepare_child(
                    frame,
                    hits,
                    ChildInteraction {
                        layer,
                        rect: tracks.disclosure,
                        target,
                        spec: disclosure_spec,
                        role: Role::TextSecondary,
                    },
                    theme,
                )
            } else {
                PointerPresentation::Rest
            };
        let create_pointer = if let (Some(hits), Some(target), Some(rect)) =
            (hits.as_deref_mut(), create_target.as_ref(), tracks.create)
        {
            prepare_child(
                frame,
                hits,
                ChildInteraction {
                    layer,
                    rect,
                    target,
                    spec: auxiliary_spec,
                    role: Role::AccentAction,
                },
                theme,
            )
        } else {
            PointerPresentation::Rest
        };
        let menu_pointer =
            if let (Some(hits), Some(target)) = (hits.as_deref_mut(), menu_target.as_ref()) {
                prepare_child(
                    frame,
                    hits,
                    ChildInteraction {
                        layer,
                        rect: tracks.menu,
                        target,
                        spec: auxiliary_spec,
                        role: Role::AccentAction,
                    },
                    theme,
                )
            } else {
                PointerPresentation::Rest
            };

        let row_target = source
            .drawn(target.clone(), SidebarPart::Primary, window)
            .map(InteractionTarget::Sidebar);
        let row_spec = interaction_spec(PrimaryContract::ActivateOnRelease, true);
        let row_pointer = if let (Some(hits), Some(target)) = (hits.as_deref(), row_target.as_ref())
        {
            pointer_presentation(hits.projected_pointer(layer, band, band, target, row_spec))
        } else {
            PointerPresentation::Rest
        };
        let mut bands = vec![
            RowContentBand::explicit(tracks.disclosure),
            RowContentBand::explicit(tracks.current),
            RowContentBand::explicit(tracks.state),
            RowContentBand::explicit(tracks.unread),
        ];
        if !tracks.ordinal.is_empty() {
            bands.push(RowContentBand::explicit(tracks.ordinal));
        }
        if !tracks.auxiliary.is_empty() {
            bands.push(RowContentBand::explicit(tracks.auxiliary));
        }
        let painted = paint_row_interaction(
            frame,
            band,
            RowTracks::new(RowFocusCue::EnclosingFrame, tracks.primary)
                .with_provisional(tracks.navigation),
            &bands,
            ControlSurface::Chrome,
            row_state(owns_keyboard, focused, current, row_pointer),
            theme,
        );
        let interaction = winning_surface(row_pointer, focused, current);
        let suffix = &annotations[index];
        let text_width = usize::from(tracks.primary.width);
        let inline = if suffix_rows(suffix, text_width) == 0 {
            suffix.as_str()
        } else {
            ""
        };
        let projected_lines = match row {
            SidebarRow::Project { label, .. } => {
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
                ProjectedSidebarLines::new(
                    vec![Line::styled(
                        format!(
                            "{}{inline}",
                            truncate_display(
                                &display_label(&name),
                                text_width.saturating_sub(display_width(inline)),
                                theme.glyphs.ellipsis,
                            )
                        ),
                        theme.style(Role::TextPrimary).add_modifier(Modifier::BOLD),
                    )],
                    CurrentIdentityEmphasis::Theme,
                )
            }
            SidebarRow::Context { row, .. } => lines(view, row, locale, theme, text_width, inline),
            SidebarRow::Agent { row, .. } => {
                super::agent_sidebar::tokens::projection(view, row, locale, theme, text_width)
            }
        };
        let (mut row_lines, current_identity) = projected_lines.into_parts();
        if matches!(row, SidebarRow::Agent { .. }) && !inline.is_empty() {
            if row_lines.is_empty() {
                row_lines.push(Line::default());
            }
            row_lines[0] = fit(
                std::mem::take(&mut row_lines[0]),
                text_width.saturating_sub(display_width(inline)),
                theme.glyphs.ellipsis,
            );
            row_lines[0].spans.push(Span::styled(
                inline.to_owned(),
                theme.style(Role::TextSecondary),
            ));
        }
        row_lines.extend(summary_lines(view, row, locale, theme, text_width));
        append_identity(
            &mut row_lines,
            suffix,
            text_width,
            continuation_style(view, row, theme),
        );
        for line in &mut row_lines {
            *line = fit(std::mem::take(line), text_width, theme.glyphs.ellipsis);
        }
        compose_lines(&mut row_lines, row_pointer, focused, current, theme);
        frame.render_widget(
            Paragraph::new(row_lines),
            Rect::new(
                tracks.primary.x,
                tracks.content_y,
                tracks.primary.width,
                content_height,
            ),
        );

        let disclosure = match row {
            SidebarRow::Project { .. } => {
                Some(if view.machines.sidebar_project_expanded(&target) {
                    theme.glyphs.disclosure_open
                } else {
                    theme.glyphs.disclosure_closed
                })
            }
            SidebarRow::Context {
                has_agents: true, ..
            } => Some(if view.machines.sidebar_context_agents_expanded(&target) {
                theme.glyphs.disclosure_open
            } else {
                theme.glyphs.disclosure_closed
            }),
            SidebarRow::Context {
                has_agents: false, ..
            }
            | SidebarRow::Agent { .. } => None,
        };
        if let Some(disclosure) = disclosure {
            frame.render_widget(
                Paragraph::new(disclosure).style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Role::TextSecondary),
                    theme.style(Role::TextSecondary),
                )),
                tracks.first_line(tracks.disclosure),
            );
            let _ = paint_pointer_target(
                frame,
                tracks.disclosure,
                ControlSurface::Chrome,
                disclosure_pointer,
                Role::TextSecondary,
                Availability::Enabled,
                theme,
            );
        }

        if let SidebarRow::Agent { ordinal, .. } = row
            && *ordinal < 9
        {
            frame.render_widget(
                Paragraph::new(format!("{}", ordinal + 1)).style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Role::HintLabel),
                    theme.style(Role::HintLabel),
                )),
                tracks.first_line(tracks.ordinal),
            );
        }
        let state = match row {
            SidebarRow::Context { row, .. } => view.machines.workspace_state(row).map(|state| {
                let (_, role) = state_role(state);
                (sidebar_text::state_glyph(state, theme), role)
            }),
            SidebarRow::Agent { row, .. } => row.sidebar_agent().map(|agent| {
                let state = agent.display_state();
                let (_, role) = state_role(state);
                (sidebar_text::state_glyph(state, theme), role)
            }),
            SidebarRow::Project { .. } => None,
        };
        if let Some((glyph, role)) = state {
            frame.render_widget(
                Paragraph::new(glyph).style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(role),
                    theme.style(role),
                )),
                tracks.first_line(tracks.state),
            );
        }
        let unread = match row {
            SidebarRow::Agent { row, .. } => row.sidebar_agent().is_some_and(|agent| !agent.seen),
            SidebarRow::Context { row, .. } => view.machines.workspace_unread(row),
            SidebarRow::Project { .. } => false,
        };
        let collapsed_fill_marker = theme.current_context_marker();
        if current
            && (current_identity == CurrentIdentityEmphasis::ExplicitPlain
                || !collapsed_fill_marker.trim().is_empty())
        {
            frame.render_widget(
                Paragraph::new(theme.glyphs.active_marker).style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Role::TextPrimary),
                    theme.style(Role::TextPrimary),
                )),
                tracks.first_line(tracks.current),
            );
        }
        if unread {
            frame.render_widget(
                Paragraph::new(theme.glyphs.status_bullet).style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Role::StatusUnread),
                    theme.style(Role::StatusUnread),
                )),
                tracks.first_line(tracks.unread),
            );
        }

        let density = tracks.density();
        if let (Some(target), Some(create)) = (create_target.as_ref(), tracks.create) {
            let label = if density == layout::Density::Comfortable {
                locale.machine_picker_text("sidebar-create")
            } else {
                "+"
            };
            frame.render_widget(
                Paragraph::new(padded_action(label, None, usize::from(create.width))).style(
                    theme.content_on_surface(
                        Role::SurfaceChrome,
                        interaction,
                        Some(Role::HintLabel),
                        theme.style(Role::HintLabel),
                    ),
                ),
                tracks.first_line(create),
            );
            let _ = paint_pointer_target(
                frame,
                create,
                ControlSurface::Chrome,
                create_pointer,
                Role::AccentAction,
                Availability::Enabled,
                theme,
            );
            let _ = target;
        }
        if let Some(target) = menu_target.as_ref() {
            let label = if density == layout::Density::Comfortable {
                locale.machine_picker_text("sidebar-menu")
            } else {
                theme.glyphs.ellipsis
            };
            let shortcut = (density == layout::Density::Comfortable && owns_keyboard)
                .then(|| {
                    crate::machine_picker::sidebar_binding_label(
                        crate::machine_picker::SidebarCommand::OpenMenu,
                        super::agent_sidebar::key_label_glyphs(theme),
                    )
                })
                .flatten();
            frame.render_widget(
                Paragraph::new(padded_action(
                    label,
                    shortcut.as_deref(),
                    usize::from(tracks.menu.width),
                ))
                .style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Role::HintLabel),
                    theme.style(Role::HintLabel),
                )),
                tracks.first_line(tracks.menu),
            );
            let _ = paint_pointer_target(
                frame,
                tracks.menu,
                ControlSurface::Chrome,
                menu_pointer,
                Role::AccentAction,
                Availability::Enabled,
                theme,
            );
            let _ = target;
        }

        restore_interaction_background(frame, band, interaction, theme);
        if disclosure_target.is_some() {
            let _ = paint_pointer_target(
                frame,
                tracks.disclosure,
                ControlSurface::Chrome,
                disclosure_pointer,
                Role::TextSecondary,
                Availability::Enabled,
                theme,
            );
        }
        if create_target.is_some()
            && let Some(create) = tracks.create
        {
            let _ = paint_pointer_target(
                frame,
                create,
                ControlSurface::Chrome,
                create_pointer,
                Role::AccentAction,
                Availability::Enabled,
                theme,
            );
        }
        if menu_target.is_some() {
            let _ = paint_pointer_target(
                frame,
                tracks.menu,
                ControlSurface::Chrome,
                menu_pointer,
                Role::AccentAction,
                Availability::Enabled,
                theme,
            );
        }
        if painted.is_some()
            && let (Some(hits), Some(target)) = (hits.as_deref_mut(), row_target)
        {
            hits.record_interaction(layer, band, band, target, row_spec);
        }
        y = band.bottom();
    }

    if let Some(hits) = hits.as_deref_mut() {
        hits.record(
            layer,
            Rect::new(body.x, y, body.width, body.bottom().saturating_sub(y)),
            crate::tui_pointer::HitTarget::MachineWorkspaceSurface(window),
        );
    }
    let range = if window.top > 0 || window.end() < window.total {
        format!(
            "{}-{}/{}",
            if window.capacity > 0 {
                window.top + 1
            } else {
                0
            },
            window.end(),
            window.total,
        )
    } else {
        String::new()
    };
    if range.is_empty() {
        return;
    }
    let footer = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    let label = format!(
        "{} {range}",
        locale.machine_picker_text("workspace-sidebar-open")
    );
    let target = InteractionTarget::SidebarFixed(
        view.machines
            .drawn_sidebar_fixed(source, SidebarFixedAction::Browse),
    );
    let spec = interaction_spec(PrimaryContract::ActivateOnRelease, view.machines.available);
    let base_state = InteractionState::new(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        if view.machines.available {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        OperationState::Idle,
    );
    let rest = ActionSpec::new(
        &label,
        ActionKind::Secondary,
        ControlSurface::Chrome,
        base_state,
    );
    let Some(rect) = action_rect(frame.area(), footer, rest, theme) else {
        return;
    };
    let pointer = hits.as_deref().map_or(PointerPresentation::Rest, |hits| {
        pointer_presentation(hits.projected_pointer(layer, rect, rect, &target, spec))
    });
    let painted = render_action(
        frame,
        footer,
        ActionSpec::new(
            &label,
            ActionKind::Secondary,
            ControlSurface::Chrome,
            base_state.with_pointer(pointer),
        ),
        theme,
    );
    if painted == Some(rect)
        && let Some(hits) = hits
    {
        hits.record_interaction(layer, rect, rect, target, spec);
    }
}
#[cfg(test)]
mod tests;
