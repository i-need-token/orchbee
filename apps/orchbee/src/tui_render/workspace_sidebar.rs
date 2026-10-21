//! Workspace token rows over complete canonical checkpoints and attention-projected Agent states.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, NavigationState, OperationState,
    PointerPresentation, RowContentBand, RowFocusCue, RowTracks, action_rect, icon_action_rect,
    paint_pointer_target, paint_row_interaction, render_action, render_icon_action,
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

/// The fixed `[x]` icon-action extent revealed flush with a closeable row's trailing edge.
const CLOSE_WIDTH: u16 = 3;
/// Cells a closeable identity leaves free so a revealed close never abuts it.
const CLOSE_GAP: usize = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::tui_render) enum CurrentIdentityEmphasis {
    Theme,
    ExplicitPlain,
}

pub(in crate::tui_render) struct ProjectedSidebarLines {
    lines: Vec<Line<'static>>,
    /// Right-aligned metadata per line; an empty line has none.
    trailing: Vec<Line<'static>>,
    current_identity: CurrentIdentityEmphasis,
}

impl ProjectedSidebarLines {
    pub(in crate::tui_render) fn new(
        lines: Vec<Line<'static>>,
        current_identity: CurrentIdentityEmphasis,
    ) -> Self {
        Self {
            lines,
            trailing: Vec::new(),
            current_identity,
        }
    }

    pub(in crate::tui_render) fn into_parts(
        self,
    ) -> (
        Vec<Line<'static>>,
        Vec<Line<'static>>,
        CurrentIdentityEmphasis,
    ) {
        (self.lines, self.trailing, self.current_identity)
    }
}

struct Context<'a> {
    row: &'a Row,
    observed: Option<&'a crate::machine_spaces::Row>,
    state: Option<&'static str>,
    role: Role,
    standalone: &'static str,
    unnamed: &'static str,
    git_stale: bool,
    branch_redundant: bool,
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
            role,
            standalone: locale.shell().standalone,
            unnamed: locale.unnamed_workspace(),
            git_stale: view.machines.space_checkpoint_stale(row),
            branch_redundant: view.machines.space_branch_redundant(row),
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
                    .map(|label| {
                        if label.trim().is_empty() {
                            self.unnamed
                        } else {
                            label
                        }
                    })
                    .unwrap_or(self.standalone),
            ),
            Kind::Branch => (!self.branch_redundant)
                .then_some(self.observed?.git.as_ref()?.branch.as_deref())
                .flatten(),
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
/// Ahead/behind counts painted as right-aligned metadata on their configured line.
fn git_spans(
    context: &Context<'_>,
    theme: &Theme,
    style: WindowsSidebarTokenStyle,
) -> Vec<Span<'static>> {
    let Some(git) = context
        .observed
        .and_then(|r| r.git.as_ref())
        .and_then(|g| g.divergence.as_ref())
    else {
        return Vec::new();
    };
    let mut spans = Vec::new();
    if git.ahead > 0 {
        spans.push(Span::styled(
            format!("{}{n}", theme.glyphs.git_ahead, n = git.ahead),
            theme.workspace_token_style(Role::StatusSuccess, style, false, false),
        ));
    }
    if git.behind > 0 {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::styled(
            format!("{}{n}", theme.glyphs.git_behind, n = git.behind),
            theme.workspace_token_style(Role::StatusError, style, false, false),
        ));
    }
    spans
}
/// `first_reserve` is the trailing cell count the first line keeps free even without metadata, so
/// the identity truncates identically at rest and while a close control is revealed.
fn lines(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
    width: usize,
    suffix: &str,
    first_reserve: usize,
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
    let slot = |metadata: usize, reserve: usize| match metadata {
        0 => reserve,
        cells => (cells + 1).max(reserve),
    };
    let mut lines = Vec::new();
    let mut trailing = Vec::new();
    let mut slots = Vec::new();
    let mut current_identity = CurrentIdentityEmphasis::Theme;
    let identity = machine.clone();
    for tokens in view.machines.space_settings().rows() {
        // The state icon has its own leading track; Git counts move to the trailing slot.
        let (git, resolved): (Vec<_>, Vec<_>) = tokens
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
                    Kind::StateIcon | Kind::GitStatus => String::new(),
                    Kind::Branch => format!(
                        "{}{}",
                        display_label(context.value(kind).expect("visible branch")),
                        if context.git_stale { " ?" } else { "  " }
                    ),
                    _ => display_label(context.value(kind).expect("visible value")),
                };
                Some((kind, style.expect("visible style"), text))
            })
            .partition(|(kind, _, _)| *kind == Kind::GitStatus);
        let mut right = Vec::new();
        for (_, style, _) in &git {
            let spans = git_spans(&context, theme, *style);
            if !spans.is_empty() && !right.is_empty() {
                right.push(Span::raw(" "));
            }
            right.extend(spans);
        }
        let right = Line::from(right);
        let line_slot = slot(
            right.width(),
            if lines.is_empty() { first_reserve } else { 0 },
        );
        let desired: Vec<_> = resolved
            .iter()
            .map(|(_, _, text)| display_width(text))
            .collect();
        let separators = resolved.len().saturating_sub(1) * 3;
        let mut remaining = width
            .saturating_sub(if lines.is_empty() {
                display_width(&identity) + display_width(suffix)
            } else {
                0
            })
            .saturating_sub(separators + line_slot);
        let mut budgets = vec![0; resolved.len()];
        // Distribute bounded text cells fairly after the trailing slot is reserved.
        loop {
            let mut assigned = false;
            for i in 0..resolved.len() {
                if remaining > 0 && budgets[i] < desired[i] {
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
        for (i, (kind, style, text)) in resolved.into_iter().enumerate() {
            let identity_token =
                kind == Kind::Workspace || lines.is_empty() && matches!(kind, Kind::Custom(_));
            if identity_token && style.bold == Some(false) {
                current_identity = CurrentIdentityEmphasis::ExplicitPlain;
            }
            let text = if kind == Kind::Branch {
                let marker = if context.git_stale { " ?" } else { "  " };
                format!(
                    "{}{}",
                    truncate_display(
                        &display_label(context.value(kind).expect("visible branch")),
                        budgets[i].saturating_sub(2),
                        theme.glyphs.ellipsis
                    ),
                    if budgets[i] >= 2 { marker } else { "" }
                )
            } else {
                truncate_display(&text, budgets[i], theme.glyphs.ellipsis)
            };
            if text.is_empty() {
                continue;
            }
            if !spans.is_empty() {
                spans.push(Span::styled(
                    format!(" {} ", theme.glyphs.middle_dot),
                    text_style(Role::TextSecondary),
                ));
            }
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
        // Configured tracks keep their height while asynchronous metadata arrives.
        if !tokens.is_empty() {
            lines.push(Line::from(spans));
            trailing.push(right);
            slots.push(line_slot);
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
        trailing.push(Line::default());
        slots.push(slot(0, first_reserve));
    }
    lines[0]
        .spans
        .insert(0, Span::styled(identity, text_style(Role::TextSecondary)));
    lines[0] = fit(
        std::mem::take(&mut lines[0]),
        width.saturating_sub(display_width(suffix) + slots[0]),
        theme.glyphs.ellipsis,
    );
    lines[0].spans.push(Span::styled(
        suffix.to_owned(),
        text_style(Role::TextSecondary),
    ));
    for (line, slot) in lines.iter_mut().zip(&slots).skip(1) {
        *line = fit(
            std::mem::take(line),
            width.saturating_sub(*slot),
            theme.glyphs.ellipsis,
        );
    }
    ProjectedSidebarLines {
        lines,
        trailing,
        current_identity,
    }
}

fn configured_height(view: &ClientView) -> u16 {
    view.machines
        .space_settings()
        .rows()
        .iter()
        .filter(|tokens| !tokens.is_empty())
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

fn annotations(
    context: EntryMeasureContext<'_>,
    rows: &[SidebarRow<'_>],
    width: u16,
) -> Vec<String> {
    let EntryMeasureContext {
        view,
        source,
        locale,
        sidebar_width,
        theme,
    } = context;
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
                if matches!(row, SidebarRow::Project { .. }) {
                    truncate_display(
                        &label(row, locale),
                        entry_width(view, source, row, width, sidebar_width),
                        theme.glyphs.ellipsis,
                    )
                } else {
                    label(row, locale)
                },
            ))
            .or_insert_with(Vec::new)
            .push((index, identity(&target).to_owned()));
    }
    let mut annotations = vec![String::new(); rows.len()];
    for peers in groups.values().filter(|peers| peers.len() > 1) {
        let identities = peers.iter().map(|(_, id)| id.as_str()).collect::<Vec<_>>();
        for (index, id) in peers {
            annotations[*index] = crate::terminal_text::unique_identity_suffix(id, &identities);
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
    layout::AuxiliaryActions::new(source.has_part(target, SidebarPart::Create))
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
    .map_or(1, |tracks| {
        usize::from(tracks.text_width(source.has_part(&target, SidebarPart::Disclosure)))
    })
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
        SidebarRow::Context { .. } => configured_height(view),
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

pub(super) struct RenderContext<'a> {
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) layer: HitLayer,
    /// The complete painted Sidebar surface, before frame and padding are removed.
    pub(super) sidebar_width: u16,
    pub(super) owns_keyboard: bool,
    pub(super) source: &'a SidebarSource,
    pub(super) creation: Option<crate::terminal_session::WorkspaceCreationPreview<'a>>,
}

fn creation_target_matches(
    view: &ClientView,
    row: &SidebarRow<'_>,
    preview: crate::terminal_session::WorkspaceCreationPreview<'_>,
) -> bool {
    let target = row.target();
    target.machine() == &view.machines.current
        && target.site_id() == preview.site
        && match target {
            SidebarTarget::Context { target, .. } => matches!(&target.location,
                crate::terminal_session::NativeContextLocation::Workspace { workspace_id, .. }
                    if preview.committed.is_some_and(|(workspace, _)| workspace_id == workspace)),
            SidebarTarget::Project { project_id, .. } => {
                preview.project_id == Some(&project_id)
                    && preview.committed.is_none_or(|(workspace, revision)| {
                        !view
                            .machines
                            .workspace_checkpoint_covers(preview.site, revision)
                            || view
                                .machines
                                .workspace_checkpoint_project(preview.site, workspace)
                                == Some(&project_id)
                    })
                    && !view.machines.sidebar_project_expanded(&row.target())
            }
            SidebarTarget::Agent { .. } => false,
        }
}

fn creation_insertion(
    view: &ClientView,
    rows: &[SidebarRow<'_>],
    preview: crate::terminal_session::WorkspaceCreationPreview<'_>,
) -> usize {
    let belongs = |row: &SidebarRow<'_>| {
        let target = row.target();
        target.machine() == &view.machines.current && target.site_id() == preview.site
    };
    if let Some(project) = preview.project_id
        && let Some(parent) = rows.iter().position(|row| {
            belongs(row)
                && matches!(row, SidebarRow::Project { project_id, .. } if *project_id == project)
        })
    {
        return rows.iter().enumerate().skip(parent + 1).find_map(|(index, row)| {
            (!belongs(row) || matches!(row, SidebarRow::Project { .. })
                || matches!(row, SidebarRow::Context { .. } if view.machines.sidebar_parent(&row.target()).is_none()))
                .then_some(index)
        }).unwrap_or(rows.len());
    }
    if let Some(index) = rows
        .iter()
        .rposition(|row| row.target().machine() == &view.machines.current)
    {
        return index + 1;
    }
    let machine_index = view
        .machines
        .rows
        .iter()
        .position(|machine| machine.choice == view.machines.current);
    rows.iter()
        .position(|row| {
            view.machines
                .rows
                .iter()
                .position(|machine| &machine.choice == row.target().machine())
                .zip(machine_index)
                .is_some_and(|(candidate, current)| candidate > current)
        })
        .unwrap_or(rows.len())
}

fn render_creation(
    frame: &mut Frame<'_>,
    row: Rect,
    sidebar_width: u16,
    preview: crate::terminal_session::WorkspaceCreationPreview<'_>,
    locale: Locale,
    theme: &Theme,
) {
    let Some(tracks) = layout::Tracks::resolve_for_width(
        row,
        0,
        u16::from(preview.project_id.is_some()),
        sidebar_width,
        layout::AuxiliaryActions::default(),
        layout::RowKind::Owner,
    ) else {
        return;
    };
    let name = super::workspace_creation::label(preview, locale);
    let pending = preview.phase != crate::terminal_session::WorkspaceCreationPhase::Committed;
    let mut lines = vec![fit(
        Line::from(vec![
            Span::styled(display_label(name), theme.style(Role::TextPrimary)),
            Span::styled(
                if pending {
                    format!(" · {}", locale.workspace_creation_progress(preview.phase))
                } else {
                    String::new()
                },
                theme.style(Role::TextSecondary),
            ),
        ]),
        usize::from(tracks.primary.width),
        theme.glyphs.ellipsis,
    )];
    let state = InteractionState::new(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
        if preview.current {
            CurrentState::Current
        } else {
            CurrentState::NotCurrent
        },
        CheckedState::Unchecked,
        Availability::Enabled,
        if pending {
            OperationState::Pending
        } else {
            OperationState::Idle
        },
    );
    let _ = paint_row_interaction(
        frame,
        row,
        RowTracks::new(RowFocusCue::EnclosingFrame, tracks.primary).with_status(tracks.state),
        &[RowContentBand::explicit(tracks.current)],
        ControlSurface::Chrome,
        state,
        theme,
    );
    sidebar_text::compose_lines(
        &mut lines,
        winning_surface(PointerPresentation::Rest, false, preview.current),
        preview.current,
        theme,
    );
    frame.render_widget(Paragraph::new(lines), tracks.primary);
    if preview.current && !theme.current_context_marker().trim().is_empty() {
        frame.render_widget(
            Paragraph::new(theme.glyphs.active_marker).style(theme.style(Role::TextPrimary)),
            tracks.current,
        );
    }
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

fn restore_interaction_background(
    frame: &mut Frame<'_>,
    rect: Rect,
    interaction: Option<Role>,
    theme: &Theme,
) {
    let Some(background) = interaction
        .and_then(|role| theme.style(role).bg)
        .or_else(|| theme.style(Role::SurfaceChrome).bg)
    else {
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

struct AuxiliaryAction<'a> {
    rect: Rect,
    density: layout::Density,
    label: &'a str,
    glyph: IconActionGlyph,
}

impl<'a> AuxiliaryAction<'a> {
    fn resolve(
        bounds: Rect,
        area: Rect,
        density: layout::Density,
        label: &'a str,
        glyph: IconActionGlyph,
        theme: &Theme,
    ) -> Option<Self> {
        let mut rect = if density == layout::Density::Comfortable {
            action_rect(
                bounds,
                area,
                Self::spec(label, PointerPresentation::Rest),
                theme,
            )
        } else {
            icon_action_rect(bounds, area)
        }?;
        rect.x = area.right().checked_sub(rect.width)?;
        if bounds.intersection(rect) != rect {
            return None;
        }
        Some(Self {
            rect,
            density,
            label,
            glyph,
        })
    }

    fn spec(label: &str, pointer: PointerPresentation) -> ActionSpec<'_> {
        ActionSpec::new(
            label,
            ActionKind::Auxiliary,
            ControlSurface::Chrome,
            InteractionState::new(
                FocusState::Unfocused,
                pointer,
                NavigationState::None,
                CurrentState::NotCurrent,
                CheckedState::Unchecked,
                Availability::Enabled,
                OperationState::Idle,
            ),
        )
    }

    fn render(
        &self,
        frame: &mut Frame<'_>,
        pointer: PointerPresentation,
        parent_surface: Option<Role>,
        theme: &Theme,
    ) {
        let painted = if self.density == layout::Density::Comfortable {
            render_action(frame, self.rect, Self::spec(self.label, pointer), theme)
        } else {
            render_icon_action(
                frame,
                self.rect,
                IconActionSpec::new(
                    self.glyph,
                    ControlSurface::Chrome,
                    Availability::Enabled,
                    pointer,
                ),
                theme,
            )
        };
        debug_assert_eq!(painted, Some(self.rect));
        if pointer == PointerPresentation::Rest && parent_surface.is_some() {
            for x in self.rect.x..self.rect.right() {
                let cell = &mut frame.buffer_mut()[(x, self.rect.y)];
                cell.set_style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    parent_surface,
                    Some(Role::TextSecondary),
                    cell.style(),
                ));
            }
        }
    }
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

// The most specific visible current row owns the fill. Its expanded ancestors retain their
// identity emphasis and marker, so a Workspace and its active Agent never form one filled block.
fn current_descendant(
    view: &ClientView,
    rows: &[SidebarRow<'_>],
    projected_current: &[bool],
    target: &SidebarTarget,
) -> bool {
    rows.iter().zip(projected_current).any(|(row, current)| {
        let descendant = row.target();
        if !*current || &descendant == target {
            return false;
        }
        let mut parent = view.machines.sidebar_parent(&descendant);
        while let Some(ancestor) = parent {
            if &ancestor == target {
                return true;
            }
            parent = view.machines.sidebar_parent(&ancestor);
        }
        false
    })
}

fn starts_group(view: &ClientView, rows: &[SidebarRow<'_>], index: usize) -> bool {
    match &rows[index] {
        SidebarRow::Project { .. } => true,
        row @ SidebarRow::Context { .. }
            if matches!(
                row.target(),
                SidebarTarget::Context {
                    target: crate::terminal_session::NativeContextTarget {
                        location: crate::terminal_session::NativeContextLocation::Workspace { .. },
                        ..
                    },
                    ..
                }
            ) && view.machines.sidebar_parent(&row.target()).is_none() =>
        {
            rows[..index]
                .iter()
                .rev()
                .find(|row| !matches!(row, SidebarRow::Agent { .. }))
                .is_some_and(|previous| {
                    matches!(previous, SidebarRow::Project { .. })
                        || view.machines.sidebar_parent(&previous.target()).is_some()
                })
        }
        _ => false,
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
        creation,
    } = context;
    if area.is_empty() {
        return;
    }
    if rows.is_empty() && creation.is_none() {
        return;
    }
    let theme = &theme.paired_pointer_surfaces();
    let extra_creation = creation.filter(|preview| {
        !rows
            .iter()
            .any(|row| creation_target_matches(view, row, *preview))
            && preview.committed.is_none_or(|(_, revision)| {
                !view
                    .machines
                    .workspace_checkpoint_covers(preview.site, revision)
            })
    });
    // A committed receipt may outlive observer handoff and member switches inside its Workspace.
    // It supplies current context only until a canonical current row is available. Unfinished
    // foreground creation still owns current feedback even while the prior member remains admitted.
    let current_creation = creation.filter(|preview| {
        preview.current
            && (preview.replaces_content()
                || !rows.iter().any(|row| current_row(view, row, &row.target())))
    });
    let projected_current: Vec<_> = rows
        .iter()
        .map(|row| match current_creation {
            Some(preview) => {
                extra_creation.is_none() && creation_target_matches(view, row, preview)
            }
            None => current_row(view, row, &row.target()),
        })
        .collect();
    let unobserved = creation.is_some_and(|preview| {
        preview.committed.is_none_or(|(_, revision)| {
            !view
                .machines
                .workspace_checkpoint_covers(preview.site, revision)
        })
    });
    let count = view.machines.sidebar_workspace_count() + usize::from(unobserved);
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
    let header_rows = if area.height >= 10 { 2 } else { 1 };
    let body = Rect::new(
        area.x,
        area.y + header_rows,
        area.width,
        area.height - header_rows - 1,
    );
    let insertion = extra_creation.map(|preview| creation_insertion(view, rows, preview));
    let measure = EntryMeasureContext {
        view,
        source,
        locale,
        sidebar_width,
        theme,
    };
    let annotations = annotations(measure, rows, area.width);
    let mut heights: Vec<_> = rows
        .iter()
        .zip(&annotations)
        .map(|(row, suffix)| entry_height(measure, row, suffix, area.width))
        .collect();
    let space_gap = view.machines.space_settings().row_gap();
    let agent_gap = view.machines.sidebar_settings().row_gap();
    let mut gaps: Vec<_> = rows
        .iter()
        .enumerate()
        .map(|(index, _)| match rows.get(index + 1) {
            Some(SidebarRow::Agent { .. }) => agent_gap,
            _ if index + 1 < rows.len() && starts_group(view, rows, index + 1) => space_gap.max(1),
            _ => space_gap,
        })
        .collect();
    if let Some(index) = insertion {
        heights.insert(index, configured_height(view));
        let following_gap = if index < rows.len() && starts_group(view, rows, index) {
            space_gap.max(1)
        } else {
            space_gap
        };
        gaps.insert(index, following_gap);
        if let Some(previous) = index.checked_sub(1) {
            let starts_ungrouped = extra_creation
                .is_some_and(|preview| preview.project_id.is_none())
                && rows[..index]
                    .iter()
                    .rev()
                    .find(|row| !matches!(row, SidebarRow::Agent { .. }))
                    .is_some_and(|row| {
                        matches!(row, SidebarRow::Project { .. })
                            || view.machines.sidebar_parent(&row.target()).is_some()
                    });
            gaps[previous] = if starts_ungrouped {
                space_gap.max(1)
            } else {
                space_gap
            };
        }
    }
    let window = view.machines.space_window_with_creation(
        rows,
        &heights,
        &gaps,
        body.height,
        insertion,
        current_creation.and_then(|preview| {
            insertion.or_else(|| {
                rows.iter()
                    .position(|row| creation_target_matches(view, row, preview))
            })
        }),
    );
    if insertion.is_none()
        && let Some(hits) = hits.as_deref_mut()
    {
        hits.record_sidebar_reorder_paint(crate::tui_pointer::SidebarReorderPaint {
            viewport: frame.area(),
            layer,
            body,
            footer: Rect::new(area.x, area.bottom() - 1, area.width, 1),
            window,
            source: source.clone(),
        });
    }
    let mut y = body.y;
    let mut hovered_hint = None;

    for index in window.top..window.end() {
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
        if insertion == Some(index) {
            if let Some(preview) = extra_creation {
                render_creation(
                    frame,
                    Rect::new(body.x, y + leading_gap, body.width, content_height),
                    sidebar_width,
                    crate::terminal_session::WorkspaceCreationPreview {
                        current: current_creation.is_some(),
                        ..preview
                    },
                    locale,
                    theme,
                );
            }
            y += leading_gap + content_height;
            continue;
        }
        let row_index = index - usize::from(insertion.is_some_and(|slot| index > slot));
        let row = &rows[row_index];
        // Project separation is outside its row: it never paints or activates as that Project.
        let group_gap = if starts_group(view, rows, row_index) {
            leading_gap
        } else {
            0
        };
        if group_gap > 0 {
            let gap = Rect::new(body.x, y, body.width, group_gap);
            frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), gap);
            if let Some(hits) = hits.as_deref_mut() {
                hits.record(
                    layer,
                    gap,
                    crate::tui_pointer::HitTarget::MachineWorkspaceSurface(window),
                );
            }
        }
        let band = Rect::new(
            body.x,
            y + group_gap,
            body.width,
            leading_gap - group_gap + content_height,
        );
        let target = row.target();
        let actions = auxiliary_actions(source, &target);
        let Some(tracks) = layout::Tracks::resolve_for_width(
            band,
            leading_gap - group_gap,
            depth(view, &target),
            sidebar_width,
            actions,
            layout::RowKind::for_target(&target),
        ) else {
            break;
        };
        let focused = owns_keyboard && view.machines.sidebar_focused_target() == Some(&target);
        let current = projected_current[row_index];
        let current_ancestor =
            current && current_descendant(view, rows, &projected_current, &target);
        let filled_current = current && !current_ancestor;
        frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), band);

        let suffix = &annotations[row_index];
        let closeable = source.has_part(&target, SidebarPart::Close);
        let has_disclosure = source.has_part(&target, SidebarPart::Disclosure);
        let text_width = usize::from(tracks.text_width(has_disclosure));
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
            SidebarRow::Context { row, .. } => lines(
                view,
                row,
                locale,
                theme,
                text_width,
                inline,
                if closeable { CLOSE_GAP } else { 0 },
            ),
            SidebarRow::Agent { row, .. } => {
                super::agent_sidebar::tokens::projection(view, row, locale, theme, text_width)
            }
        };
        let (mut row_lines, mut trailing, current_identity) = projected_lines.into_parts();
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
        let tracks = if has_disclosure {
            tracks.with_disclosure_after(row_lines.first().map_or(0, Line::width))
        } else {
            tracks
        };

        let disclosure_drawn = (!tracks.disclosure.is_empty())
            .then(|| source.drawn(target.clone(), SidebarPart::Disclosure, window))
            .flatten();
        let create_drawn = source.drawn(target.clone(), SidebarPart::Create, window);

        let disclosure_target = disclosure_drawn.map(InteractionTarget::Sidebar);
        let create_target = create_drawn.map(InteractionTarget::Sidebar);
        let create_action = tracks.create.and_then(|rect| {
            AuxiliaryAction::resolve(
                frame.area(),
                tracks.first_line(rect),
                tracks.density(),
                locale.machine_picker_text("sidebar-create"),
                IconActionGlyph::Add,
                theme,
            )
        });
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
        let create_pointer = if let (Some(hits), Some(target), Some(action)) = (
            hits.as_deref_mut(),
            create_target.as_ref(),
            create_action.as_ref(),
        ) {
            prepare_child(
                frame,
                hits,
                ChildInteraction {
                    layer,
                    rect: action.rect,
                    target,
                    spec: auxiliary_spec,
                    role: Role::TextSecondary,
                },
                theme,
            )
        } else {
            PointerPresentation::Rest
        };
        // Close sits flush with the row's trailing edge, over the first line's metadata gap and
        // current/unread marker cells, which it hides while revealed.
        let close_spec = interaction_spec(PrimaryContract::ActivateOnRelease, true);
        let close_target = source
            .drawn(target.clone(), SidebarPart::Close, window)
            .and_then(|drawn| {
                let offered = Rect::new(
                    tracks.unread.right().checked_sub(CLOSE_WIDTH)?,
                    tracks.content_y,
                    CLOSE_WIDTH,
                    1,
                );
                let rect = icon_action_rect(frame.area(), offered)?;
                (rect.x >= tracks.primary.x).then_some((rect, InteractionTarget::Sidebar(drawn)))
            });
        let close_pointer = match (hits.as_deref(), close_target.as_ref()) {
            (Some(hits), Some((rect, target))) => pointer_presentation(
                hits.projected_pointer(layer, *rect, *rect, target, close_spec),
            ),
            _ => PointerPresentation::Rest,
        };
        let row_target = source
            .drawn(target.clone(), SidebarPart::Primary, window)
            .map(InteractionTarget::Sidebar);
        let row_spec = interaction_spec(PrimaryContract::ActivateOnRelease, true);
        let row_pointer = match (hits.as_deref_mut(), row_target.as_ref()) {
            (Some(hits), Some(target)) => {
                // A revealed close was published as an exposed child of this row; projecting
                // without it would orphan the row's hover or press after the reveal frame.
                let beneath =
                    close_target
                        .as_ref()
                        .map_or(PointerPresentation::Rest, |(rect, close)| {
                            pointer_presentation(hits.projected_pointer_beneath(
                                layer,
                                (*rect, close, close_spec),
                                band,
                                band,
                                target,
                                row_spec,
                            ))
                        });
                if beneath == PointerPresentation::Rest {
                    pointer_presentation(
                        hits.projected_pointer(layer, band, band, target, row_spec),
                    )
                } else {
                    beneath
                }
            }
            _ => PointerPresentation::Rest,
        };
        let row_paint_pointer = if row_pointer == PointerPresentation::Rest
            && (disclosure_pointer != PointerPresentation::Rest
                || create_pointer != PointerPresentation::Rest
                || close_pointer != PointerPresentation::Rest)
        {
            PointerPresentation::Hovered
        } else {
            row_pointer
        };
        // Revealed only by pointer presence on the row or the control itself; at rest the
        // trailing metadata owns the slot and no close target is published.
        let close = close_target.filter(|_| row_paint_pointer != PointerPresentation::Rest);
        let expanded = match row {
            SidebarRow::Project { .. } => Some(view.machines.sidebar_project_expanded(&target)),
            SidebarRow::Context {
                has_agents: true, ..
            } => Some(view.machines.sidebar_context_agents_expanded(&target)),
            SidebarRow::Context {
                has_agents: false, ..
            }
            | SidebarRow::Agent { .. } => None,
        };
        let disclosure_glyph = |expanded| {
            if expanded {
                theme.glyphs.disclosure_open
            } else {
                theme.glyphs.disclosure_closed
            }
        };
        if row_paint_pointer != PointerPresentation::Rest {
            let mut hint = Vec::new();
            if current {
                hint.push(format!(
                    "{} {}",
                    theme.glyphs.active_marker,
                    locale.sidebar_marker_label("current")
                ));
            }
            let state = match row {
                SidebarRow::Agent { row, .. } => {
                    row.sidebar_agent().map(|agent| agent.display_state())
                }
                SidebarRow::Context { row, .. } => view.machines.workspace_state(row),
                SidebarRow::Project { .. } => None,
            };
            if let Some(state) = state {
                hint.push(format!(
                    "{} {}",
                    theme.agent_glyph(state),
                    locale.machine_picker_text(state_role(state).0)
                ));
            }
            let unread = match row {
                SidebarRow::Agent { row, .. } => {
                    row.sidebar_agent().is_some_and(|agent| !agent.seen)
                }
                SidebarRow::Context { row, .. } => view.machines.workspace_unread(row),
                SidebarRow::Project { .. } => false,
            };
            if unread {
                hint.push(format!(
                    "{} {}",
                    theme.glyphs.status_bullet,
                    locale.sidebar_marker_label("unread")
                ));
            }
            if has_disclosure && let Some(expanded) = expanded {
                hint.push(format!(
                    "{} {}",
                    disclosure_glyph(expanded),
                    locale.sidebar_marker_label(if expanded { "expanded" } else { "collapsed" })
                ));
            }
            hint.push(locale.sidebar_marker_label("help").to_owned());
            hovered_hint = Some(if close_pointer == PointerPresentation::Rest {
                hint.join(" · ")
            } else {
                locale
                    .menu_action(&crate::action_menu::MenuAction::CloseWorkspace)
                    .to_owned()
            });
        }
        let mut bands = vec![
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
            row_state(owns_keyboard, focused, filled_current, row_paint_pointer),
            theme,
        );
        let interaction = winning_surface(row_paint_pointer, focused, filled_current);
        sidebar_text::compose_lines(
            &mut row_lines,
            interaction,
            current || row_paint_pointer == PointerPresentation::Pressed,
            theme,
        );
        frame.render_widget(
            Paragraph::new(row_lines),
            Rect::new(
                tracks.primary.x,
                tracks.content_y,
                tracks.primary.width,
                content_height,
            ),
        );
        sidebar_text::compose_lines(&mut trailing, interaction, false, theme);
        for (index, metadata) in trailing.into_iter().enumerate() {
            let Ok(offset) = u16::try_from(index) else {
                break;
            };
            if offset >= content_height {
                break;
            }
            let width = u16::try_from(metadata.width()).unwrap_or(u16::MAX);
            // A revealed close control replaces first-line metadata in the same slot.
            if width == 0 || index == 0 && close.is_some() {
                continue;
            }
            let x = tracks
                .primary
                .right()
                .saturating_sub(width)
                .max(tracks.primary.x);
            frame.render_widget(
                Paragraph::new(metadata),
                Rect::new(x, tracks.content_y + offset, tracks.primary.right() - x, 1),
            );
        }

        let disclosure = expanded.map(disclosure_glyph);
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
            && close.is_none()
            && (current_ancestor
                || current_identity == CurrentIdentityEmphasis::ExplicitPlain
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
        if unread && close.is_none() {
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

        if create_target.is_some()
            && let Some(action) = create_action.as_ref()
        {
            action.render(frame, create_pointer, interaction, theme);
        }
        if let Some((rect, _)) = close.as_ref() {
            // Rest content inherits the row surface; hover and press turn it destructive below.
            let painted = render_icon_action(
                frame,
                *rect,
                IconActionSpec::new(
                    IconActionGlyph::Close,
                    ControlSurface::Chrome,
                    Availability::Enabled,
                    PointerPresentation::Rest,
                ),
                theme,
            );
            debug_assert_eq!(painted, Some(*rect));
            for x in rect.x..rect.right() {
                let cell = &mut frame.buffer_mut()[(x, rect.y)];
                cell.set_style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Role::TextSecondary),
                    cell.style(),
                ));
            }
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
            && let Some(action) = create_action.as_ref()
        {
            let _ = paint_pointer_target(
                frame,
                action.rect,
                ControlSurface::Chrome,
                create_pointer,
                Role::TextSecondary,
                Availability::Enabled,
                theme,
            );
        }
        if let Some((rect, target)) = close
            && paint_pointer_target(
                frame,
                rect,
                ControlSurface::Chrome,
                close_pointer,
                Role::AccentDestructive,
                Availability::Enabled,
                theme,
            )
            .is_some()
            && let Some(hits) = hits.as_deref_mut()
        {
            hits.record_interaction(layer, rect, rect, target, close_spec);
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
    if let Some(hint) = hovered_hint {
        frame.render_widget(
            Paragraph::new(truncate_display(
                &hint,
                usize::from(area.width),
                theme.glyphs.ellipsis,
            ))
            .style(theme.style(Role::TextSecondary)),
            Rect::new(area.x, area.bottom() - 1, area.width, 1),
        );
        return;
    }
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
