//! Workspace token rows over complete canonical checkpoints and attention-projected Agent states.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    IconActionGlyph, IconActionSpec, InteractionState, NavigationState, OperationState,
    PointerPresentation, RowContentBand, RowFocusCue, RowTracks, action_rect, icon_action_rect,
    paint_pointer_target, paint_row_interaction, render_action, render_icon_action,
};
use super::{
    Block, ClientView, Frame, HitLayer, HitMapBuilder, Line, Locale, Paragraph, Rect, Role, Span,
    Theme, display_width, sidebar_text, tooltip, truncate_display,
};
use crate::terminal_text::{line_display_width, sanitize_terminal_text};
use crate::{
    client_view::ClientAction,
    localization::TooltipLabel,
    machine_discovery::display_label,
    machine_picker::{Row, SidebarPart, SidebarRow, SidebarSource, SidebarTarget},
    tui_hit_map::{
        InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer, TooltipContent,
        TooltipText,
    },
};
use orchbee_local_runtime::{SidebarTokenStyle, SpaceSidebarToken, SpaceSidebarTokenKind as Kind};
use ratatui::style::{Modifier, Style};
use sidebar_text::{fit, machine_label, marker_cells, state_role, truncate_machine_name};
pub(super) mod agent_tokens;
mod layout;

/// The fixed `[x]` icon-action extent revealed flush with a closeable row's trailing edge.
const CLOSE_WIDTH: u16 = 3;
/// Cells a closeable identity always leaves free: the close extent and one blank cell before it,
/// so the name truncates the same at rest and while close shows.
const CLOSE_RESERVE: usize = 4;
/// The trailing unread marker and the blank cell before it.
const UNREAD_CELLS: usize = 2;

pub(in crate::tui_render) struct ProjectedSidebarLines {
    lines: Vec<Line<'static>>,
    /// Right-aligned metadata per line; an empty line has none.
    trailing: Vec<Line<'static>>,
    /// Each name a line shows, whole, by line index: what its pointer tooltip gives in full once
    /// the painted line cuts it.
    names: Vec<(usize, TooltipText)>,
    /// The folder a Workspace's location names, which gives its full directory under the pointer.
    folder: Option<FolderPart>,
    /// A quiet Workspace name's span on the first line and the primary style the pointer raises it
    /// to.
    raise_name: Option<(usize, Style)>,
}

/// Where a line names a folder, in cells from the line's start, and the directory it stands for.
struct FolderPart {
    line: usize,
    start: usize,
    width: usize,
    directory: TooltipText,
}

impl ProjectedSidebarLines {
    /// Paints every cell muted, glyphs included: a row of a machine that is not connected shows
    /// what it was when last seen.
    fn mute(&mut self, theme: &Theme) {
        let muted = theme.style(Role::TextMuted);
        for line in self.lines.iter_mut().chain(&mut self.trailing) {
            line.style = line.style.patch(muted);
            for span in &mut line.spans {
                span.style = span.style.patch(muted);
            }
        }
    }

    pub(in crate::tui_render) fn new(lines: Vec<Line<'static>>) -> Self {
        Self {
            lines,
            trailing: Vec::new(),
            names: Vec::new(),
            folder: None,
            raise_name: None,
        }
    }

    /// Lines that show these names, whole, by line index.
    pub(in crate::tui_render) fn with_names(mut self, names: Vec<(usize, TooltipText)>) -> Self {
        self.names = names;
        self
    }

    #[cfg(test)]
    pub(in crate::tui_render) fn names(&self) -> &[(usize, TooltipText)] {
        &self.names
    }

    pub(in crate::tui_render) fn into_parts(self) -> (Vec<Line<'static>>, Vec<Line<'static>>) {
        (self.lines, self.trailing)
    }

    /// Each line's first name that its painted line no longer shows whole, by line index.
    fn cut_names(
        names: Vec<(usize, TooltipText)>,
        painted: &[Line<'_>],
    ) -> Vec<(usize, TooltipText)> {
        let mut cut: Vec<(usize, TooltipText)> = Vec::new();
        for (line, name) in names {
            let Some(painted) = painted.get(line) else {
                continue;
            };
            let text: String = painted
                .spans
                .iter()
                .map(|span| span.content.as_ref())
                .collect();
            if !text.contains(&name.text) && cut.iter().all(|(other, _)| *other != line) {
                cut.push((line, name));
            }
        }
        cut
    }
}

/// A name shown whole, cut at its end.
pub(in crate::tui_render) fn full_name(text: impl Into<String>) -> TooltipText {
    TooltipText {
        text: text.into(),
        middle: false,
    }
}

/// How a Workspace's projected lines leave room for trailing cells and a folded status line.
#[derive(Clone, Copy, Default)]
struct LineShape {
    /// Cells only the identity line keeps for the disclosure that follows the name; continuation
    /// lines use the whole width.
    disclosure: usize,
    /// Cells the identity line gives a shown unread marker.
    markers: usize,
    /// Cells the identity line keeps free at rest, so a revealed close never re-truncates it.
    reserve: usize,
    /// The status lines a folded Workspace's Agent glyphs take in place of its second line, which
    /// push its later lines down; zero while its Agent rows show.
    status: u16,
    /// A Workspace that is neither current, nor above its current Agent, nor navigated names itself
    /// in secondary text, as the GUI's does, until the pointer raises it.
    quiet: bool,
}

struct Context<'a> {
    row: &'a Row,
    observed: Option<&'a crate::machine_spaces::Row>,
    state: Option<&'static str>,
    role: Role,
    no_workspace: &'static str,
    unnamed: &'static str,
    /// What this row keeps until the Git facts of its new directory are known.
    held: Option<&'a crate::machine_spaces::HeldRow>,
    /// The directory `location` stands for when no branch is observed: the first terminal's
    /// current directory, else the Workspace's own directory.
    directory: Option<&'a str>,
    /// That directory by its folder's name, never the whole path.
    folder: Option<String>,
    /// The state word of the row's machine while it is not connected.
    unconnected: Option<&'static str>,
}
impl<'a> Context<'a> {
    fn new(view: &'a ClientView, row: &'a Row, locale: Locale) -> Self {
        let glyph_state = view.machines.workspace_state(row);
        let (key, role) =
            state_role(glyph_state.unwrap_or(orchbee_workbench_protocol::MetadataState::Unknown));
        let observed = view.machines.space_checkpoint_row(row);
        let held = view.machines.space_held_row(row);
        let directory = held
            .map_or_else(
                || observed.and_then(crate::machine_spaces::Row::source_path),
                |held| held.folder.as_ref(),
            )
            .map(orchbee_workbench_protocol::WorkbenchDirectoryPath::as_str);
        let home = (row.choice == crate::machine_picker::Choice::Local)
            .then(local_home)
            .flatten();
        let folder = directory.map(|path| folder_name(path, home));
        Self {
            row,
            observed,
            state: glyph_state.map(|_| locale.machine_picker_text(key)),
            role,
            no_workspace: locale.shell().no_workspace,
            unnamed: locale.unnamed_workspace(),
            held,
            directory,
            folder,
            unconnected: view
                .machines
                .unconnected_state(&row.choice)
                .map(|key| locale.machine_picker_text(key)),
        }
    }
    fn git(&self) -> Option<&'a crate::machine_spaces::Git> {
        match self.held {
            Some(held) => held.git.as_ref(),
            None => self.observed?.git.as_ref(),
        }
    }
    fn branch(&self) -> Option<&'a str> {
        self.git()?.branch.as_deref()
    }
    /// Whether `value` only repeats the Workspace's name: the name itself, or the numbered name the
    /// Server gives a second Workspace named after the same folder, as `Desktop (2)`.
    fn repeats_name(&self, value: &str) -> bool {
        let Some(name) = self.value(Kind::Workspace).map(str::trim) else {
            return false;
        };
        let value = value.trim();
        name == value
            || name
                .strip_prefix(value)
                .and_then(|rest| rest.strip_prefix(" ("))
                .and_then(|rest| rest.strip_suffix(')'))
                .is_some_and(|ordinal| {
                    !ordinal.is_empty() && ordinal.bytes().all(|byte| byte.is_ascii_digit())
                })
    }
    /// The branch a branch or location line shows: none when it repeats the Workspace's name, as a
    /// worktree named after its branch does, so the word appears once. A location line then gives
    /// the folder instead.
    fn distinct_branch(&self) -> Option<&'a str> {
        self.branch().filter(|branch| !self.repeats_name(branch))
    }
    /// The folder a location line shows: none when it repeats the Workspace's name, as a Workspace
    /// named after its folder does, so that line leaves the row.
    fn distinct_folder(&self) -> Option<&str> {
        self.folder
            .as_deref()
            .filter(|folder| !self.repeats_name(folder))
    }
    /// Branch text is a name, while a folder location is a path.
    fn shows_branch(&self, kind: Kind<'_>) -> bool {
        kind == Kind::Branch || kind == Kind::Location && self.distinct_branch().is_some()
    }
    /// A continuation line appears only once one of its tokens has a visible value; pending
    /// metadata reserves no blank row, so a non-repository Workspace never shrinks after Git settles.
    fn keeps(&self, tokens: &[SpaceSidebarToken], first: bool) -> bool {
        !tokens.is_empty()
            && (first
                || tokens.iter().any(|token| {
                    let kind = token.kind();
                    kind != Kind::StateIcon
                        && self.visible(kind, token.style_for_value(self.value(kind)))
                }))
    }
    fn value(&self, kind: Kind<'_>) -> Option<&str> {
        match kind {
            Kind::StateIcon | Kind::GitStatus => None,
            Kind::StateText => self.state,
            Kind::Workspace => Some(
                self.held
                    .map_or_else(
                        || self.observed.and_then(|r| r.label.as_deref()),
                        |held| held.label.as_deref(),
                    )
                    .or_else(|| self.row.workspace_label())
                    .map(|label| {
                        if label.trim().is_empty() {
                            self.unnamed
                        } else {
                            label
                        }
                    })
                    .unwrap_or(self.no_workspace),
            ),
            Kind::Branch => self.distinct_branch(),
            Kind::Location => self.distinct_branch().or(self.distinct_folder()),
            Kind::Custom(key) => self
                .observed?
                .tokens
                .as_map()
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v.as_str()),
        }
    }
    fn visible(&self, kind: Kind<'_>, style: Option<SidebarTokenStyle>) -> bool {
        if style.is_none() {
            return false;
        }
        match kind {
            Kind::StateIcon => true,
            Kind::GitStatus => self
                .git()
                .and_then(|g| g.divergence.as_ref())
                .is_some_and(|g| g.ahead > 0 || g.behind > 0),
            _ => self
                .value(kind)
                .is_some_and(|v| !display_label(v).is_empty()),
        }
    }
}
/// The Client's home folder, read once; only a local Workspace can stand in it.
fn local_home() -> Option<&'static str> {
    static HOME: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    HOME.get_or_init(|| {
        std::env::home_dir().and_then(|home| home.into_os_string().into_string().ok())
    })
    .as_deref()
}
/// A directory by its last folder's name, so a location line never shows a whole path. The home
/// folder itself reads `~`, compared under Windows spelling equivalence: either separator, ASCII
/// case and trailing separators. A root has no folder name and keeps its own spelling.
fn folder_name(path: &str, home: Option<&str>) -> String {
    const SEPARATORS: [char; 2] = ['\\', '/'];
    let trimmed = path.trim_end_matches(SEPARATORS);
    let separator = |byte: u8| matches!(byte, b'\\' | b'/');
    if let Some(home) = home.map(|home| home.trim_end_matches(SEPARATORS))
        && !home.is_empty()
        && trimmed.len() == home.len()
        && trimmed
            .bytes()
            .zip(home.bytes())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b) || separator(a) && separator(b))
    {
        return "~".to_owned();
    }
    trimmed
        .rsplit(SEPARATORS)
        .next()
        .filter(|name| !name.is_empty() && !name.ends_with(':'))
        .map_or_else(|| path.to_owned(), str::to_owned)
}
/// Ahead/behind counts painted as right-aligned metadata on their configured line. Like common
/// Git tools, commits to push read green and commits to pull read as a caution; the arrows still
/// tell them apart where a host shows no color.
fn git_spans(context: &Context<'_>, theme: &Theme, style: SidebarTokenStyle) -> Vec<Span<'static>> {
    let Some(git) = context.git().and_then(|g| g.divergence.as_ref()) else {
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
            theme.workspace_token_style(Role::StatusWarning, style, false, false),
        ));
    }
    spans
}
/// The identity line keeps `shape.reserve` cells free even without metadata, so it truncates
/// identically at rest and while a close control is revealed. Every token, a folder's name
/// included, truncates at its end; a folder's full directory belongs to its pointer tooltip.
fn lines(
    view: &ClientView,
    row: &Row,
    locale: Locale,
    theme: &Theme,
    width: usize,
    shape: LineShape,
) -> ProjectedSidebarLines {
    let context = Context::new(view, row, locale);
    let identity_width = width.saturating_sub(shape.disclosure);
    let text_style = |role| theme.workspace_token_style(role, Default::default(), false, false);
    // A Workspace on another machine names it after its own name, as `api @devbox`.
    let marker = (row.choice != crate::machine_picker::Choice::Local)
        .then(|| machine_label(view, row, locale));
    let marker_color = view.machines.machine_color(&row.choice);
    // Metadata keeps one blank cell before it; the identity line also leaves the unread cells.
    let slot = |metadata: usize| match metadata {
        0 => 0,
        cells => cells + 1,
    };
    let first_slot = |metadata: usize| (slot(metadata) + shape.markers).max(shape.reserve);
    let mut lines = Vec::new();
    let mut trailing = Vec::new();
    let mut slots = Vec::new();
    let mut names = Vec::new();
    let mut folder = None;
    let mut raise_name = None;
    let name_role = if shape.quiet {
        Role::TextSecondary
    } else {
        Role::TextPrimary
    };
    // The span of the identity line that holds the machine's name, painted in its color.
    let mut marker_spans = None;
    for tokens in view.machines.space_settings().rows() {
        if lines.is_empty() {
            marker_spans = None;
        }
        // A Workspace row has no state column, so a configured state icon paints nothing; Git
        // counts move to the trailing slot.
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
        let line_slot = if lines.is_empty() {
            first_slot(line_display_width(&right))
        } else {
            slot(line_display_width(&right))
        };
        let desired: Vec<_> = resolved
            .iter()
            .map(|(_, _, text)| display_width(text))
            .collect();
        let separators = resolved.len().saturating_sub(1) * 3;
        let mut remaining = if lines.is_empty() {
            identity_width
        } else {
            width
        }
        .saturating_sub(separators + line_slot);
        // The marker takes one blank cell and `@` beside the Workspace name on the first line.
        let marker = marker
            .as_deref()
            .filter(|_| lines.is_empty())
            .and_then(|label| {
                let name = resolved
                    .iter()
                    .position(|(kind, _, _)| *kind == Kind::Workspace)?;
                let available = remaining.saturating_sub(2);
                let cells = marker_cells(
                    display_width(label),
                    desired.iter().sum(),
                    desired[name].min(4),
                    available,
                );
                remaining = available.saturating_sub(cells);
                Some(format!(
                    " @{}",
                    truncate_machine_name(label, cells, theme.glyphs.ellipsis)
                ))
            });
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
        let mut line_names = Vec::new();
        let mut line_folder = None;
        for (i, (kind, style, text)) in resolved.into_iter().enumerate() {
            // A Workspace name and a branch give themselves in full once cut; a folder's name
            // gives its whole directory instead, wherever the pointer rests on it.
            if kind == Kind::Workspace || context.shows_branch(kind) {
                line_names.push(full_name(text.clone()));
            }
            let text = truncate_display(&text, budgets[i], theme.glyphs.ellipsis);
            if text.is_empty() {
                continue;
            }
            if !spans.is_empty() {
                spans.push(Span::styled(
                    format!(" {} ", theme.glyphs.middle_dot),
                    text_style(if lines.is_empty() {
                        Role::TextSecondary
                    } else {
                        Role::TextMuted
                    }),
                ));
            }
            if kind == Kind::Location
                && !context.shows_branch(kind)
                && let Some(directory) = context.directory
            {
                line_folder.get_or_insert((
                    spans
                        .iter()
                        .map(|span: &Span<'_>| display_width(&span.content))
                        .sum::<usize>(),
                    display_width(&text),
                    directory,
                ));
            }
            // A location or branch, and every later line, are quiet supporting text under the name.
            let role = match kind {
                Kind::StateIcon | Kind::StateText => context.role,
                Kind::Branch | Kind::Location => Role::TextMuted,
                _ if !lines.is_empty() => Role::TextMuted,
                Kind::Workspace => name_role,
                _ => Role::TextPrimary,
            };
            if kind == Kind::Workspace && lines.is_empty() && shape.quiet {
                raise_name = Some((
                    spans.len(),
                    theme.workspace_token_style(Role::TextPrimary, style, false, false),
                ));
            }
            spans.push(Span::styled(
                text,
                theme.workspace_token_style(role, style, false, false),
            ));
            if kind == Kind::Workspace
                && let Some(marker) = marker.as_ref()
            {
                // Regular weight: the name alone reads bold on the current row. The machine's name
                // takes its color; the blank cell and `@` before it stay plain.
                let plain = text_style(name_role).remove_modifier(Modifier::BOLD);
                match (marker_color, marker.split_at_checked(2)) {
                    (Some(color), Some((lead, name))) => {
                        spans.push(Span::styled(lead.to_owned(), plain));
                        spans.push(Span::styled(
                            name.to_owned(),
                            theme.machine_name_style(color, None, plain),
                        ));
                        marker_spans = Some(spans.len() - 1);
                    }
                    _ => spans.push(Span::styled(marker.clone(), plain)),
                }
            }
        }
        // Configured tracks keep their height while asynchronous metadata arrives.
        if context.keeps(tokens, lines.is_empty()) {
            names.extend(line_names.into_iter().map(|name| (lines.len(), name)));
            if let Some((start, width, directory)) = line_folder
                && folder.is_none()
            {
                folder = Some(FolderPart {
                    line: lines.len(),
                    start,
                    width,
                    directory: TooltipText {
                        text: sanitize_terminal_text(directory).into_owned(),
                        middle: true,
                    },
                });
            }
            lines.push(Line::from(spans));
            trailing.push(right);
            slots.push(line_slot);
        }
    }
    if lines.is_empty() {
        lines.push(Line::default());
        trailing.push(Line::default());
        slots.push(first_slot(0));
    }
    if let Some(word) = context.unconnected {
        // A machine that is not connected: the identity line reads muted, and its state word
        // takes the second line in place of the location and the folded Agents' states.
        lines.truncate(1);
        trailing.truncate(1);
        slots.truncate(1);
        names.retain(|(line, _)| *line == 0);
        folder = folder.filter(|part| part.line == 0);
        let muted = theme.style(Role::TextMuted);
        for (index, span) in lines[0].spans.iter_mut().enumerate() {
            // The machine's name keeps its hue, at the row's reduced strength.
            span.style = if marker_spans == Some(index) {
                span.style.add_modifier(Modifier::DIM)
            } else {
                span.style.patch(muted)
            };
        }
        trailing[0] = Line::default();
        lines.push(Line::from(Span::styled(
            display_label(word),
            text_style(Role::TextSecondary),
        )));
        trailing.push(Line::default());
        slots.push(0);
    } else if shape.status > 0 {
        // The renderer paints the Agent glyphs over these reserved lines: the second line, then one
        // more for each line they wrap onto, ahead of the later configured lines.
        let extra = usize::from(shape.status - 1);
        names.retain(|(line, _)| *line != 1);
        for (line, _) in &mut names {
            if *line > 1 {
                *line += extra;
            }
        }
        folder = folder.filter(|part| part.line != 1).map(|mut part| {
            if part.line > 1 {
                part.line += extra;
            }
            part
        });
        if lines.len() > 1 {
            lines[1] = Line::default();
            trailing[1] = Line::default();
            slots[1] = 0;
        } else {
            lines.push(Line::default());
            trailing.push(Line::default());
            slots.push(0);
        }
        for _ in 0..extra {
            lines.insert(2, Line::default());
            trailing.insert(2, Line::default());
            slots.insert(2, 0);
        }
    }
    lines[0] = fit(
        std::mem::take(&mut lines[0]),
        identity_width.saturating_sub(slots[0]),
        theme.glyphs.ellipsis,
    );
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
        names,
        folder,
        raise_name,
    }
}

/// A folded Workspace has its Agents' status lines under its name, in place of its second line, and
/// any configured line after that follows them.
fn context_height(view: &ClientView, row: &Row, locale: Locale, status: u16) -> u16 {
    let context = Context::new(view, row, locale);
    if context.unconnected.is_some() {
        return 2;
    }
    let mut height = 0;
    for tokens in view.machines.space_settings().rows() {
        if context.keeps(tokens, height == 0) {
            height += 1;
        }
    }
    match status {
        0 => height.max(1),
        lines => height.max(2) + lines - 1,
    }
}

/// A Workspace whose Agents are folded away lists their states under its name.
fn folded(view: &ClientView, row: &SidebarRow<'_>) -> bool {
    matches!(
        row,
        SidebarRow::Context {
            has_agents: true,
            ..
        }
    ) && !view.machines.sidebar_context_agents_expanded(&row.target())
}

/// How many Agent glyphs one status line holds in `width` cells: one cell each, with one blank
/// cell between neighbors.
fn status_per_line(width: u16) -> usize {
    usize::from(width).div_ceil(2)
}

/// The status lines a folded Workspace takes for `agents` glyphs in `width` cells. Every Agent
/// shows, so the glyphs wrap onto further lines; the first line stays even before any Agent is
/// known.
fn status_lines(agents: usize, width: u16) -> u16 {
    match status_per_line(width) {
        0 => 1,
        per_line => u16::try_from(agents.div_ceil(per_line))
            .unwrap_or(u16::MAX)
            .max(1),
    }
}

/// Where a folded Workspace's `agents` glyphs stand in `area`, its status lines: in order, one
/// cell each with one blank cell between neighbors, wrapping onto the next line. Glyphs below the
/// area's last line are cut off with the row.
fn status_cells(area: Rect, agents: usize) -> Vec<Rect> {
    let per_line = status_per_line(area.width);
    if per_line == 0 {
        return Vec::new();
    }
    (0..agents)
        .map_while(|index| {
            let line = u16::try_from(index / per_line)
                .ok()
                .filter(|line| *line < area.height)?;
            let column = u16::try_from(2 * (index % per_line)).ok()?;
            Some(Rect::new(area.x + column, area.y + line, 1, 1))
        })
        .collect()
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

fn label(row: &SidebarRow<'_>, locale: Locale) -> String {
    display_label(match row {
        SidebarRow::Project { label, .. } => {
            if label.is_empty() {
                locale.shell().projects
            } else {
                label
            }
        }
        SidebarRow::Recents { .. } => locale.machine_picker_text("sidebar-recents"),
        SidebarRow::Context { row, .. } => {
            row.workspace_label().unwrap_or(locale.shell().no_workspace)
        }
        SidebarRow::Agent { row, .. } => row
            .sidebar_agent()
            .map_or("", crate::machine_agents::Row::title),
    })
}

fn auxiliary_actions(source: &SidebarSource, target: &SidebarTarget) -> layout::AuxiliaryActions {
    layout::AuxiliaryActions::new(source.has_part(target, SidebarPart::Create))
}

/// Names repeat as they are: no row grows a line, or an ID, to tell same-named peers apart. A folded
/// Workspace grows by its `status` lines past the first.
fn entry_height(view: &ClientView, row: &SidebarRow<'_>, locale: Locale, status: u16) -> u16 {
    match row {
        SidebarRow::Project { .. } | SidebarRow::Recents { .. } => 1,
        SidebarRow::Context { row: context, .. } => context_height(view, context, locale, status),
        SidebarRow::Agent { row, .. } => agent_tokens::height(view, row),
    }
}

pub(super) struct RenderContext<'a> {
    pub(super) native: Option<&'a crate::workbench_client::WorkbenchClientView<'a>>,
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) layer: HitLayer,
    /// The complete painted Sidebar surface, before frame and padding are removed.
    pub(super) sidebar_width: u16,
    /// Padding cells after `area` that row surfaces continue through, up to the divider.
    pub(super) trailing_padding: u16,
    pub(super) owns_keyboard: bool,
    pub(super) source: &'a SidebarSource,
    pub(super) creation:
        Option<crate::workbench_state::workspace_setup::WorkspaceCreationPreview<'a>>,
    /// The row whose pointer-opened menu is open keeps its hover presentation.
    pub(super) held: Option<&'a SidebarTarget>,
}

fn creation_target_matches(
    view: &ClientView,
    row: &SidebarRow<'_>,
    preview: crate::workbench_state::workspace_setup::WorkspaceCreationPreview<'_>,
) -> bool {
    let target = row.target();
    target.machine() == &view.machines.current
        && target.session_id() == preview.session
        && match target {
            SidebarTarget::Context { target, .. } => matches!(&target.location,
                crate::workbench_projection::ClientContextLocation::Workspace { workspace_id, .. }
                    if preview.committed.is_some_and(|(workspace, _)| workspace_id == workspace)),
            // A folded destination heading stands in for the hidden provisional row.
            SidebarTarget::Project { .. } | SidebarTarget::Recents { .. } => {
                let project_id = match &target {
                    SidebarTarget::Project { project_id, .. } => Some(project_id),
                    _ => None,
                };
                preview.project_id == project_id
                    && preview.committed.is_none_or(|(workspace, revision)| {
                        !view
                            .machines
                            .workspace_checkpoint_covers(preview.session, revision)
                            || view
                                .machines
                                .workspace_checkpoint_group(preview.session, workspace)
                                == Some(project_id)
                    })
                    && !view.machines.sidebar_group_expanded(&target)
            }
            SidebarTarget::Agent { .. } => false,
        }
}

fn creation_insertion(
    view: &ClientView,
    rows: &[SidebarRow<'_>],
    preview: crate::workbench_state::workspace_setup::WorkspaceCreationPreview<'_>,
) -> usize {
    let belongs = |row: &SidebarRow<'_>| {
        let target = row.target();
        target.machine() == &view.machines.current && target.session_id() == preview.session
    };
    if let Some(project) = preview.project_id
        && let Some(parent) = rows.iter().position(|row| {
            belongs(row)
                && matches!(row, SidebarRow::Project { project_id, .. } if *project_id == project)
        })
    {
        return rows
            .iter()
            .enumerate()
            .skip(parent + 1)
            .find_map(|(index, row)| (!belongs(row) || starts_group(rows, index)).then_some(index))
            .unwrap_or(rows.len());
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
    preview: crate::workbench_state::workspace_setup::WorkspaceCreationPreview<'_>,
    locale: Locale,
    theme: &Theme,
) {
    let Some(tracks) = layout::Tracks::resolve_for_width(
        row,
        0,
        sidebar_width,
        layout::AuxiliaryActions::default(),
        layout::RowKind::Owner,
    ) else {
        return;
    };
    // The row names the Workspace alone; the content region carries creation progress.
    let name = super::workspace_creation::label(preview, locale);
    let mut lines = vec![fit(
        Line::from(Span::styled(
            display_label(name),
            theme.style(Role::TextPrimary),
        )),
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
        OperationState::Idle,
    );
    let _ = paint_row_interaction(
        frame,
        row,
        RowTracks::new(RowFocusCue::EnclosingFrame, tracks.primary).with_plain_current(),
        &[],
        ControlSurface::Chrome,
        state,
        theme,
    );
    let interaction = winning_surface(PointerPresentation::Rest, false, preview.current);
    sidebar_text::compose_lines(&mut lines, interaction, false, theme);
    frame.render_widget(Paragraph::new(lines), tracks.primary);
    paint_workspace_mark(
        frame,
        tracks.first_line(tracks.disclosure),
        preview.current,
        Role::TextSecondary,
        interaction,
        theme,
    );
}

/// The mark a Workspace without Agents leads its name with in its disclosure's place, solid and
/// primary while it is current, otherwise in `role`.
fn paint_workspace_mark(
    frame: &mut Frame<'_>,
    rect: Rect,
    current: bool,
    role: Role,
    interaction: Option<Role>,
    theme: &Theme,
) {
    let (glyph, role) = if current {
        (theme.glyphs.workspace_mark_current, Role::TextPrimary)
    } else {
        (theme.glyphs.workspace_mark, role)
    };
    frame.render_widget(
        Paragraph::new(glyph).style(theme.content_on_surface(
            Role::SurfaceChrome,
            interaction,
            Some(role),
            theme.style(role),
        )),
        rect,
    );
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

/// Continues a row's interaction surface through the trailing padding cells after it.
fn extend_surface(
    frame: &mut Frame<'_>,
    surface: Rect,
    padding: u16,
    interaction: Option<Role>,
    theme: &Theme,
) {
    let Some(interaction) = interaction else {
        return;
    };
    let padding =
        Rect::new(surface.right(), surface.y, padding, surface.height).intersection(frame.area());
    let style = theme.content_on_surface(
        Role::SurfaceChrome,
        Some(interaction),
        None,
        theme.style(Role::SurfaceChrome),
    );
    for y in padding.y..padding.bottom() {
        for x in padding.x..padding.right() {
            frame.buffer_mut()[(x, y)].set_style(style);
        }
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
    // A reverse band leaves hue glyphs on their contrast pair; only cleared blanks rejoin it.
    let band = interaction
        .map(|role| theme.style(role))
        .filter(|style| style.add_modifier.contains(Modifier::REVERSED));
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            let cell = &mut frame.buffer_mut()[(x, y)];
            match band {
                Some(band) => {
                    if !cell.modifier.contains(Modifier::REVERSED)
                        && cell.symbol().trim().is_empty()
                    {
                        cell.set_style(band);
                    }
                }
                None => {
                    cell.set_bg(background);
                }
            }
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

    /// Whether this action paints its own fill or band, which no row surface may replace.
    fn filled(&self, theme: &Theme) -> bool {
        self.density == layout::Density::Comfortable && theme.filled_actions()
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
        if pointer == PointerPresentation::Rest && parent_surface.is_some() && !self.filled(theme) {
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

/// Group headings only fold: they are never current, even around the current Workspace.
fn current_row(view: &ClientView, row: &SidebarRow<'_>) -> bool {
    match row {
        SidebarRow::Context { row, .. } => view.machines.current_workspace(row),
        SidebarRow::Agent { row, .. } => row
            .sidebar_agent()
            .is_some_and(|agent| view.machines.current_agent(&row.choice, agent)),
        SidebarRow::Project { .. } | SidebarRow::Recents { .. } => false,
    }
}

/// The row filled as current: the displayed Workspace, or in its place the Workspace a drag holds
/// and then the one a finished drag is still switching to.
fn shown_current(view: &ClientView, row: &SidebarRow<'_>) -> bool {
    match view.machines.sidebar_current_override() {
        Some(held) => matches!(row, SidebarRow::Context { .. }) && row.target() == *held,
        None => current_row(view, row),
    }
}

/// Project and Recents headings open the groups every Workspace belongs to.
fn starts_group(rows: &[SidebarRow<'_>], index: usize) -> bool {
    matches!(
        rows[index],
        SidebarRow::Project { .. } | SidebarRow::Recents { .. }
    )
}

/// Blank rows above `index`: an Agent row sits directly under the row above it, groups and the
/// entry after an Agent list are always separated, and a group's first Workspace sits directly
/// under its heading.
fn gap_before(view: &ClientView, rows: &[SidebarRow<'_>], index: usize, space_gap: u16) -> u16 {
    let row = &rows[index];
    if matches!(row, SidebarRow::Agent { .. }) {
        0
    } else if starts_group(rows, index)
        || index > 0 && matches!(rows[index - 1], SidebarRow::Agent { .. })
    {
        // A blank line also closes a Workspace's Agent list before the next entry.
        space_gap.max(1)
    } else if index > 0
        && starts_group(rows, index - 1)
        && view.machines.sidebar_parent(&row.target()).as_ref() == Some(&rows[index - 1].target())
    {
        0
    } else {
        space_gap
    }
}

/// The muted guide spans every child line and intervening gap without horizontal branches.
/// `stretch` gives the lines and style of the guide's heavier stretch beside the current Agent's
/// content lines, in the selection accent, `accent-focus`, which the GUI's `border-selected`
/// matches. The guide sits on the row's `interaction` surface.
fn paint_tree_guide(
    frame: &mut Frame<'_>,
    band: Rect,
    tracks: layout::Tracks,
    follows_sibling: bool,
    stretch: Option<(u16, Style)>,
    interaction: Option<Role>,
    theme: &Theme,
) {
    let on_chrome = |role, content| {
        theme.content_on_surface(Role::SurfaceChrome, interaction, Some(role), content)
    };
    let (stretch_height, stretch_style) = stretch.unwrap_or_default();
    let stretch_lines = tracks.content_y..tracks.content_y.saturating_add(stretch_height);
    for y in band.y..band.bottom() {
        if y >= tracks.content_y || follows_sibling {
            let (glyph, style) = if stretch_lines.contains(&y) {
                (
                    theme.glyphs.tree_guide_current,
                    on_chrome(Role::AccentFocus, stretch_style),
                )
            } else {
                (
                    theme.glyphs.tree_guide,
                    on_chrome(Role::FrameGuide, theme.style(Role::FrameGuide)),
                )
            };
            frame.render_widget(
                Paragraph::new(glyph).style(style),
                Rect::new(tracks.guide.x, y, tracks.guide.width, 1),
            );
        }
    }
}

/// A group heading uses muted text for its name, with no member count: it only groups the rows
/// under it, so it stays quieter than their supporting lines.
fn group_heading(name: &str, width: usize, theme: &Theme) -> Line<'static> {
    Line::from(Span::styled(
        truncate_display(name, width, theme.glyphs.ellipsis),
        theme.style(Role::TextMuted),
    ))
}

/// A Project or Recents heading's label; a heading groups every machine's Workspaces.
fn group_label(row: &SidebarRow<'_>, locale: Locale) -> String {
    display_label(&label(row, locale))
}

/// Paints the list into `area`. With `halos`, the list is spaced for touch: each entry owns one
/// blank line after its text, a tap there counts for it, and its fill keeps to its text lines
/// while `halos` collects them for the halves the blank lines show.
pub(super) fn render(
    frame: &mut Frame<'_>,
    area: Rect,
    context: RenderContext<'_>,
    rows: &[SidebarRow<'_>],
    mut halos: Option<&mut super::sidebar_touch::Halos>,
    mut hits: Option<&mut HitMapBuilder>,
) {
    let RenderContext {
        native,
        view,
        locale,
        theme,
        layer,
        sidebar_width,
        trailing_padding,
        owns_keyboard,
        source,
        creation,
        held,
    } = context;
    if area.is_empty() {
        return;
    }
    if rows.is_empty() && creation.is_none() {
        return;
    }
    // The fill alone marks the current row; where it collapses onto chrome, rows underline.
    let theme = &theme
        .paired_pointer_surfaces()
        .underline_collapsed_current();
    let extra_creation = creation.filter(|preview| {
        !rows
            .iter()
            .any(|row| creation_target_matches(view, row, *preview))
            && preview.committed.is_none_or(|(_, revision)| {
                !view
                    .machines
                    .workspace_checkpoint_covers(preview.session, revision)
            })
    });
    // A committed receipt may outlive observer handoff and member switches inside its Workspace.
    // It supplies current context only until a canonical current row is available. Unfinished
    // foreground creation still owns current feedback even while the prior member remains admitted.
    let current_creation = creation.filter(|preview| {
        preview.current
            && (preview.replaces_content() || !rows.iter().any(|row| current_row(view, row)))
    });
    // A folded destination heading reveals the hidden creation without taking its current fill.
    let projected_current: Vec<_> = rows
        .iter()
        .map(|row| match current_creation {
            Some(preview) => {
                extra_creation.is_none()
                    && !row.target().is_group()
                    && creation_target_matches(view, row, preview)
            }
            None => shown_current(view, row),
        })
        .collect();
    if area.height < 2 {
        return;
    }
    // The list opens directly under the fixed actions; Project and Recents headings name groups.
    let insertion = extra_creation.map(|preview| creation_insertion(view, rows, preview));
    // Touch spacing gives every entry its own trailing blank line, which replaces the configured
    // gaps; only groups keep one more line between them.
    let touch = u16::from(halos.is_some());
    // A folded Workspace's glyphs wrap at its continuation width, which every line but its first
    // uses whole.
    let status: Vec<_> = rows
        .iter()
        .map(|row| match row {
            SidebarRow::Context { row: context, .. } if folded(view, row) => {
                let target = row.target();
                let width = layout::Tracks::resolve_for_width(
                    Rect::new(area.x, area.y, area.width, 1),
                    0,
                    sidebar_width,
                    auxiliary_actions(source, &target),
                    layout::RowKind::for_target(&target),
                )
                .map_or(0, |tracks| tracks.text_width(false));
                status_lines(view.machines.workspace_agent_rows(context).len(), width)
            }
            _ => 0,
        })
        .collect();
    let mut heights: Vec<_> = rows
        .iter()
        .zip(&status)
        .map(|(row, status)| entry_height(view, row, locale, *status) + touch)
        .collect();
    let space_gap = if touch > 0 {
        0
    } else {
        view.machines.space_settings().row_gap()
    };
    let mut gaps: Vec<_> = (0..rows.len())
        .map(|index| {
            if index + 1 < rows.len() {
                gap_before(view, rows, index + 1, space_gap)
            } else {
                space_gap
            }
        })
        .collect();
    if let Some(index) = insertion {
        heights.insert(index, configured_height(view) + touch);
        let following_gap = if index < rows.len() && starts_group(rows, index) {
            space_gap.max(1)
        } else {
            space_gap
        };
        gaps.insert(index, following_gap);
        if let Some(previous) = index.checked_sub(1) {
            // The first Workspace outside any Project leaves the preceding Project's group.
            let leaves_project = extra_creation.is_some_and(|preview| preview.project_id.is_none())
                && rows[..index]
                    .iter()
                    .rev()
                    .find(|row| !matches!(row, SidebarRow::Agent { .. }))
                    .is_some_and(|row| {
                        matches!(row, SidebarRow::Project { .. })
                            || matches!(
                                view.machines.sidebar_parent(&row.target()),
                                Some(SidebarTarget::Project { .. })
                            )
                    });
            gaps[previous] = if leaves_project {
                space_gap.max(1)
            } else {
                space_gap
            };
        }
    }
    let creation_index = current_creation.and_then(|preview| {
        insertion.or_else(|| {
            rows.iter()
                .position(|row| creation_target_matches(view, row, preview))
        })
    });
    let window_in = |cells| {
        view.machines.space_window_with_creation(
            rows,
            &heights,
            &gaps,
            cells,
            insertion,
            creation_index,
        )
    };
    // The list runs down to Settings whether or not it fits; an overflowing list shows where it is
    // with the scrollbar in the trailing padding column, never with a row of its own.
    let body = area;
    let (window, header) = sticky_window(rows, &heights, &gaps, insertion, area.height, window_in);
    let mut y = body.y;

    let first = header.unwrap_or(window.top);
    for index in header.into_iter().chain(window.top..window.end()) {
        let leading_gap = if index == first {
            0
        } else {
            gaps[index - 1].min(body.bottom().saturating_sub(y))
        };
        let room = body.bottom().saturating_sub(y).saturating_sub(leading_gap);
        let content_height = (heights[index] - touch).min(room);
        if content_height == 0 {
            break;
        }
        // The blank line a touch-spaced entry owns after its text.
        let touch_line = touch.min(room - content_height);
        if insertion == Some(index) {
            if let Some(preview) = extra_creation {
                let row = Rect::new(body.x, y + leading_gap, body.width, content_height);
                let current = current_creation.is_some();
                render_creation(
                    frame,
                    row,
                    sidebar_width,
                    crate::workbench_state::workspace_setup::WorkspaceCreationPreview {
                        current,
                        ..preview
                    },
                    locale,
                    theme,
                );
                let interaction = winning_surface(PointerPresentation::Rest, false, current);
                extend_surface(frame, row, trailing_padding, interaction, theme);
                if let Some(halos) = halos.as_deref_mut() {
                    halos.fill(
                        theme,
                        Rect::new(row.x, row.y, row.width + trailing_padding, row.height),
                        interaction,
                    );
                }
            }
            y += leading_gap + content_height + touch_line;
            continue;
        }
        let row_index = index - usize::from(insertion.is_some_and(|slot| index > slot));
        let row = &rows[row_index];
        // Group separation is outside its row: it never paints or activates as that heading.
        let group_gap = if starts_group(rows, row_index) {
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
                    crate::tui_hit_map::HitTarget::MachineWorkspaceSurface(window),
                );
            }
        }
        // Spacing between Workspaces is an inert list separator, so current and hovered surfaces
        // cover only the entry itself. Agent spacing stays inside its row: the tree trunk
        // continues through it.
        let separator = if matches!(row, SidebarRow::Agent { .. }) {
            0
        } else {
            leading_gap - group_gap
        };
        if separator > 0 {
            let gap = Rect::new(body.x, y + group_gap, body.width, separator);
            frame.render_widget(Block::new().style(theme.style(Role::SurfaceChrome)), gap);
            if let Some(hits) = hits.as_deref_mut() {
                hits.record(
                    layer,
                    gap,
                    crate::tui_hit_map::HitTarget::MachineWorkspaceSurface(window),
                );
            }
        }
        let band = Rect::new(
            body.x,
            y + group_gap + separator,
            body.width,
            leading_gap - group_gap - separator + content_height,
        );
        // The row's target: its band and the blank line it owns after its text.
        let hit_band = Rect::new(band.x, band.y, band.width, band.height + touch_line);
        let target = row.target();
        let actions = auxiliary_actions(source, &target);
        let Some(tracks) = layout::Tracks::resolve_for_width(
            band,
            leading_gap - group_gap - separator,
            sidebar_width,
            actions,
            layout::RowKind::for_target(&target),
        ) else {
            break;
        };
        let focused = owns_keyboard && view.machines.sidebar_focused_target() == Some(&target);
        let current = projected_current[row_index];
        // The current row takes the current fill. While the current Agent's row shows beneath its
        // Workspace, the Workspace leaves the fill to it, since a second fill right above would join
        // the two into one block. A noncurrent Workspace takes the quieter related surface while
        // one of its Agents holds keyboard navigation, so that tint never reads as current.
        let current_agent_below = matches!(row, SidebarRow::Context { .. })
            && rows[row_index + 1..]
                .iter()
                .zip(&projected_current[row_index + 1..])
                .take_while(|(next, _)| matches!(next, SidebarRow::Agent { .. }))
                .any(|(_, current)| *current);
        let current_surface = current && !current_agent_below;
        let related = !current
            && matches!(row, SidebarRow::Context { .. })
            && owns_keyboard
            && view
                .machines
                .sidebar_focused_target()
                .filter(|focused| matches!(focused, SidebarTarget::Agent { .. }))
                .and_then(|focused| view.machines.sidebar_parent(focused))
                .as_ref()
                == Some(&target);
        frame.render_widget(
            Block::new().style(theme.style(Role::SurfaceChrome)),
            hit_band,
        );

        let expanded = match row {
            SidebarRow::Project { .. } | SidebarRow::Recents { .. } => {
                Some(view.machines.sidebar_group_expanded(&target))
            }
            SidebarRow::Context {
                has_agents: true, ..
            } => Some(view.machines.sidebar_context_agents_expanded(&target)),
            SidebarRow::Context {
                has_agents: false, ..
            }
            | SidebarRow::Agent { .. } => None,
        };
        // A Workspace repeats its Agents' unread marker only while those Agent rows are hidden.
        let unread = match row {
            SidebarRow::Agent { row, .. } => row.sidebar_agent().is_some_and(|agent| {
                !agent.seen
                    && !native.is_some_and(|native| {
                        native.will_see_agent(&agent.target.physical, agent.sequence)
                    })
            }),
            SidebarRow::Context { row, .. } => {
                expanded != Some(true)
                    && view.machines.workspace_unread(row, |agent| {
                        native.is_some_and(|native| {
                            native.will_see_agent(&agent.target.physical, agent.sequence)
                        })
                    })
            }
            SidebarRow::Project { .. } | SidebarRow::Recents { .. } => false,
        };
        let markers = if unread { UNREAD_CELLS } else { 0 };
        let status_agents = match row {
            SidebarRow::Context { row: context, .. }
                if folded(view, row)
                    && view.machines.unconnected_state(&context.choice).is_none() =>
            {
                view.machines.workspace_agent_rows(context)
            }
            _ => Vec::new(),
        };

        let closeable = source.has_part(&target, SidebarPart::Close);
        let has_disclosure = source.has_part(&target, SidebarPart::Disclosure);
        // Only the identity line leaves room for the disclosure after the name.
        let text_width = usize::from(tracks.text_width(has_disclosure));
        let continuation_width = usize::from(tracks.text_width(false));
        let mut projected_lines = match row {
            SidebarRow::Project { .. } | SidebarRow::Recents { .. } => {
                let label = group_label(row, locale);
                ProjectedSidebarLines::new(vec![group_heading(&label, text_width, theme)])
                    .with_names(vec![(0, full_name(sanitize_terminal_text(&label)))])
            }
            SidebarRow::Context { row: context, .. } => lines(
                view,
                context,
                locale,
                theme,
                continuation_width,
                LineShape {
                    disclosure: continuation_width - text_width,
                    markers,
                    reserve: if closeable { CLOSE_RESERVE } else { 0 },
                    status: status[row_index],
                    quiet: !current && !current_agent_below && !focused,
                },
            ),
            SidebarRow::Agent { row, .. } => {
                let mut projected = agent_tokens::projection(
                    view,
                    row,
                    locale,
                    theme,
                    text_width.saturating_sub(markers),
                    current,
                );
                if view.machines.unconnected_state(&row.choice).is_some() {
                    projected.mute(theme);
                }
                projected
            }
        };
        let names = std::mem::take(&mut projected_lines.names);
        let folder = projected_lines.folder.take();
        let raise_name = projected_lines.raise_name.take();
        let (mut row_lines, mut trailing) = projected_lines.into_parts();
        // An Agent title leaves the trailing unread cells only while that marker shows.
        if matches!(row, SidebarRow::Agent { .. }) && markers > 0 {
            if row_lines.is_empty() {
                row_lines.push(Line::default());
            }
            row_lines[0] = fit(
                std::mem::take(&mut row_lines[0]),
                text_width.saturating_sub(markers),
                theme.glyphs.ellipsis,
            );
        }
        for (index, line) in row_lines.iter_mut().enumerate() {
            let width = if index == 0 {
                text_width
            } else {
                continuation_width
            };
            *line = fit(std::mem::take(line), width, theme.glyphs.ellipsis);
        }
        // The row gives its first cut name; a later line that cuts its own gives that from its
        // own cells.
        let mut cut_names = ProjectedSidebarLines::cut_names(names, &row_lines).into_iter();
        let cut_name = cut_names.next().map(|(_, name)| name);
        let line_names: Vec<_> = cut_names
            .filter_map(|(line, name)| {
                let line = u16::try_from(line)
                    .ok()
                    .filter(|line| *line < content_height)?;
                Some((
                    Rect::new(band.x, tracks.content_y + line, band.width, 1),
                    name,
                ))
            })
            .collect();
        // A folder's name gives its whole directory from the cells its line still shows.
        let folder = folder.and_then(|part| {
            let line = u16::try_from(part.line)
                .ok()
                .filter(|line| *line < content_height)?;
            let end = (part.start + part.width).min(line_display_width(row_lines.get(part.line)?));
            let start = part.start.min(end);
            Some((
                Rect::new(
                    tracks.primary.x + u16::try_from(start).ok()?,
                    tracks.content_y + line,
                    u16::try_from(end - start).ok()?,
                    1,
                ),
                part.directory,
            ))
        });
        let tracks = if has_disclosure {
            tracks.with_disclosure_after(row_lines.first().map_or(0, line_display_width))
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
        // Each Agent glyph on a folded Workspace's status lines opens its own Agent.
        let status_spec = interaction_spec(PrimaryContract::ActivateOnRelease, true);
        let status_glyphs: Vec<_> = status_cells(
            Rect::new(
                tracks.primary.x,
                tracks.content_y + 1,
                tracks.primary.width,
                status[row_index].min(content_height.saturating_sub(1)),
            ),
            status_agents.len(),
        )
        .into_iter()
        .zip(&status_agents)
        .map(|(cell, agent)| {
            let drawn = source
                .drawn(
                    SidebarTarget::Agent {
                        machine: target.machine().clone(),
                        target: Box::new(agent.target.clone()),
                    },
                    SidebarPart::Status,
                    window,
                )
                .map(InteractionTarget::Sidebar);
            (cell, *agent, drawn)
        })
        .collect();
        let status_children = status_glyphs
            .iter()
            .filter(|(_, _, drawn)| drawn.is_some())
            .count();

        // Close sits flush with the row's trailing edge, over the first line's trailing metadata
        // or unread marker, which it hides while revealed.
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
        // Only a Project or Recents heading's disclosure folds it. The heading takes no fill that
        // would join the Workspace surfaces directly beneath it, and an open group shows its
        // disclosure only under pointer presence or keyboard focus, so the heading reads as plain
        // text above its Workspaces' leading disclosures.
        let heading = matches!(row, SidebarRow::Project { .. } | SidebarRow::Recents { .. });
        let disclosure_rests = !heading || expanded == Some(false);
        // The status glyphs, then disclosure, Create and close are children recorded in this order
        // before the row. A frame publishes a child only when it also paints it, so every pointer
        // is projected against the exposure its reveal frame published: the pointer-revealed set,
        // then the set shown without a pointer (the status glyphs, which always show, keyboard
        // focus, or a disclosure that rests shown). Projecting without them would orphan hover or
        // press after a reveal.
        let revealed_children: Vec<_> = status_glyphs
            .iter()
            .filter_map(|(cell, _, drawn)| drawn.as_ref().map(|drawn| (*cell, drawn, status_spec)))
            .chain(
                [
                    disclosure_target
                        .as_ref()
                        .map(|target| (tracks.disclosure, target, disclosure_spec)),
                    create_target
                        .as_ref()
                        .zip(create_action.as_ref())
                        .map(|(target, action)| (action.rect, target, auxiliary_spec)),
                    close_target
                        .as_ref()
                        .map(|(rect, target)| (*rect, target, close_spec)),
                ]
                .into_iter()
                .flatten(),
            )
            .collect();
        // The resting set is always a prefix of the revealed order; close is pointer-only.
        let resting = if focused {
            revealed_children.len() - usize::from(close_target.is_some())
        } else {
            status_children + usize::from(disclosure_target.is_some() && disclosure_rests)
        };
        let project =
            |hits: &mut HitMapBuilder,
             children: &[(Rect, &InteractionTarget, InteractionSpec)],
             (rect, target, spec): (Rect, &InteractionTarget, InteractionSpec)| {
                [children, &revealed_children[..resting.min(children.len())]]
                    .into_iter()
                    .map(|beneath| {
                        pointer_presentation(
                            hits.projected_pointer_beneath(
                                layer, beneath, rect, rect, target, spec,
                            ),
                        )
                    })
                    .find(|pointer| *pointer != PointerPresentation::Rest)
                    .unwrap_or(PointerPresentation::Rest)
            };
        let child_pointer = |hits: &mut Option<&mut HitMapBuilder>,
                             target: Option<&InteractionTarget>| {
            let hits = hits.as_deref_mut()?;
            let index = revealed_children
                .iter()
                .position(|(_, child, _)| Some(*child) == target)?;
            Some(project(
                hits,
                &revealed_children[..index],
                revealed_children[index],
            ))
        };
        let disclosure_pointer = child_pointer(&mut hits, disclosure_target.as_ref())
            .unwrap_or(PointerPresentation::Rest);
        let create_pointer =
            child_pointer(&mut hits, create_target.as_ref()).unwrap_or(PointerPresentation::Rest);
        let close_pointer = child_pointer(&mut hits, close_target.as_ref().map(|(_, close)| close))
            .unwrap_or(PointerPresentation::Rest);
        let status_pointers: Vec<_> = status_glyphs
            .iter()
            .map(|(_, _, drawn)| {
                child_pointer(&mut hits, drawn.as_ref()).unwrap_or(PointerPresentation::Rest)
            })
            .collect();
        let row_target = source
            .drawn(target.clone(), SidebarPart::Primary, window)
            .map(InteractionTarget::Sidebar);
        let row_spec = interaction_spec(PrimaryContract::ActivateOnRelease, true);
        let row_pointer = match (hits.as_deref_mut(), row_target.as_ref()) {
            (Some(hits), Some(target)) => {
                project(hits, &revealed_children, (hit_band, target, row_spec))
            }
            _ => PointerPresentation::Rest,
        };
        let held = held == Some(&target);
        let row_paint_pointer = if held
            || row_pointer == PointerPresentation::Rest
                && (disclosure_pointer != PointerPresentation::Rest
                    || create_pointer != PointerPresentation::Rest
                    || close_pointer != PointerPresentation::Rest
                    || status_pointers
                        .iter()
                        .any(|pointer| *pointer != PointerPresentation::Rest))
        {
            PointerPresentation::Hovered
        } else {
            row_pointer
        };
        let surface_pointer = match row {
            SidebarRow::Project { .. } | SidebarRow::Recents { .. } => PointerPresentation::Rest,
            SidebarRow::Agent { .. } | SidebarRow::Context { .. } => row_paint_pointer,
        };
        // Pointer presence anywhere on a foldable row, the glyph included, lights its disclosure
        // glyph, which never takes a fill of its own.
        let disclosure_role = if row_paint_pointer == PointerPresentation::Rest {
            Role::TextSecondary
        } else {
            Role::TextPrimary
        };
        // Revealed only by pointer presence on the row or the control itself; at rest the
        // trailing metadata owns the slot and no close target is published.
        let close = close_target.filter(|_| row_paint_pointer != PointerPresentation::Rest);
        let disclosure_glyph = |expanded| {
            if expanded {
                theme.glyphs.disclosure_open
            } else {
                theme.glyphs.disclosure_closed
            }
        };
        // Pointer presence or keyboard focus reveals Create, and an open group's disclosure; a
        // Workspace's disclosure always shows, so every foldable Workspace reads as one at rest.
        let revealed = row_paint_pointer != PointerPresentation::Rest || focused;
        let create_target = create_target.filter(|_| revealed);
        let disclosure_target = disclosure_target.filter(|_| disclosure_rests || revealed);
        // Only Agent rows carry a state glyph. All Agent Working glyphs share the Client's
        // animation phase with the Tab strip and pane titles.
        let agent_state = match row {
            SidebarRow::Agent { row, .. } => row
                .sidebar_agent()
                .map(crate::machine_agents::Row::display_state),
            SidebarRow::Context { .. }
            | SidebarRow::Project { .. }
            | SidebarRow::Recents { .. } => None,
        };
        // Only Agents have state and connector tracks; only occupied tracks become bands.
        let bands: Vec<_> = [tracks.state, tracks.guide, tracks.auxiliary]
            .into_iter()
            .filter(|rect| !rect.is_empty())
            .map(RowContentBand::explicit)
            .collect();
        let painted = paint_row_interaction(
            frame,
            band,
            RowTracks::new(RowFocusCue::EnclosingFrame, tracks.primary)
                .with_provisional(tracks.navigation)
                .with_plain_current(),
            &bands,
            ControlSurface::Chrome,
            row_state(owns_keyboard, focused, current_surface, surface_pointer),
            theme,
        );
        let interaction = winning_surface(surface_pointer, focused, current_surface)
            .or(related.then_some(Role::SurfaceRelated));
        let surface = tracks.surface();
        // A pointed noncurrent Agent's muted title rises to primary over its hover fill.
        let pointed_agent = matches!(row, SidebarRow::Agent { row, .. }
            if !current
                && row_paint_pointer != PointerPresentation::Rest
                && row.sidebar_agent().is_some()
                && view.machines.unconnected_state(&row.choice).is_none());
        if pointed_agent {
            agent_tokens::raise_title(&mut row_lines, theme);
        }
        // The pointer raises a quiet Workspace name to primary, as it does an Agent's title.
        if let Some((index, style)) = raise_name
            && row_paint_pointer != PointerPresentation::Rest
            && let Some(name) = row_lines
                .first_mut()
                .and_then(|line| line.spans.get_mut(index))
        {
            name.style = style;
        }
        sidebar_text::compose_lines(
            &mut row_lines,
            interaction,
            row_paint_pointer == PointerPresentation::Pressed,
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
        let unread_marker = unread && close.is_none();
        // First-line metadata sits flush with the row's trailing edge, where close appears,
        // unless the unread marker and its blank cell occupy the last two cells.
        let first_metadata_right = if unread_marker {
            tracks.primary.right().saturating_sub(UNREAD_CELLS as u16)
        } else {
            tracks.primary.right()
        };
        sidebar_text::compose_lines(&mut trailing, interaction, false, theme);
        for (index, metadata) in trailing.into_iter().enumerate() {
            let Ok(offset) = u16::try_from(index) else {
                break;
            };
            if offset >= content_height {
                break;
            }
            let width = u16::try_from(line_display_width(&metadata)).unwrap_or(u16::MAX);
            // A revealed close control replaces first-line metadata in the same slot.
            if width == 0 || index == 0 && close.is_some() {
                continue;
            }
            let right = if index == 0 {
                first_metadata_right
            } else {
                tracks.primary.right()
            };
            let x = right.saturating_sub(width).max(tracks.primary.x);
            frame.render_widget(
                Paragraph::new(metadata),
                Rect::new(x, tracks.content_y + offset, right - x, 1),
            );
        }

        let disclosure = expanded
            .filter(|_| disclosure_rests || revealed)
            .map(disclosure_glyph);
        if let Some(disclosure) = disclosure {
            frame.render_widget(
                Paragraph::new(disclosure).style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(disclosure_role),
                    theme.style(disclosure_role),
                )),
                tracks.first_line(tracks.disclosure),
            );
        }

        if matches!(
            row,
            SidebarRow::Context {
                has_agents: false,
                ..
            }
        ) {
            paint_workspace_mark(
                frame,
                tracks.first_line(tracks.disclosure),
                current,
                disclosure_role,
                interaction,
                theme,
            );
        }

        if let Some(state) = agent_state {
            frame.render_widget(
                Paragraph::new(super::sidebar_text::state_glyph(
                    state,
                    theme,
                    view.machines.working_frame(),
                ))
                .style(theme.content_on_surface(
                    Role::SurfaceChrome,
                    interaction,
                    Some(Theme::agent_role(state)),
                    theme.agent_style(state, view.machines.working_frame()),
                )),
                tracks.first_line(tracks.state),
            );
        }
        if unread_marker {
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

        // A bracketed Create inherits the row surface here; a filled one paints last, below.
        if create_target.is_some()
            && let Some(action) = create_action.as_ref()
            && !action.filled(theme)
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
        if matches!(row, SidebarRow::Agent { .. }) && !tracks.guide.is_empty() {
            // The accent's color alone: a host-relative theme marks focus with emphasis that a
            // guide glyph never takes.
            let guide_accent = Style::new().fg(theme.color(Role::AccentFocus));
            paint_tree_guide(
                frame,
                band,
                tracks,
                row_index
                    .checked_sub(1)
                    .is_some_and(|previous| matches!(rows[previous], SidebarRow::Agent { .. })),
                current.then_some((content_height, guide_accent)),
                interaction,
                theme,
            );
            // The guide runs on through the blank line before a sibling Agent.
            if touch_line > 0
                && rows
                    .get(row_index + 1)
                    .is_some_and(|next| matches!(next, SidebarRow::Agent { .. }))
            {
                paint_tree_guide(
                    frame,
                    Rect::new(band.x, band.bottom(), band.width, touch_line),
                    tracks,
                    true,
                    None,
                    None,
                    theme,
                );
            }
        }
        restore_interaction_background(frame, surface, interaction, theme);
        extend_surface(frame, surface, trailing_padding, interaction, theme);
        if let Some(halos) = halos.as_deref_mut() {
            halos.fill(
                theme,
                Rect::new(
                    surface.x,
                    surface.y,
                    surface.width + trailing_padding,
                    surface.height,
                ),
                interaction,
            );
            halos.text(band);
        }
        if let Some(target) = disclosure_target
            && paint_pointer_target(
                frame,
                tracks.disclosure,
                ControlSurface::Chrome,
                PointerPresentation::Rest,
                disclosure_role,
                Availability::Enabled,
                theme,
            )
            .is_some()
            && let Some(hits) = hits.as_deref_mut()
        {
            // Only a Workspace's disclosure names an action no row text already says.
            if matches!(row, SidebarRow::Context { .. })
                && let Some(expanded) = expanded
            {
                hits.record_interaction_tooltip(
                    layer,
                    tracks.disclosure,
                    &target,
                    TooltipContent::Action {
                        label: locale
                            .tooltip_label(if expanded {
                                TooltipLabel::HideAgents
                            } else {
                                TooltipLabel::ShowAgents
                            })
                            .to_owned(),
                        key: None,
                    },
                );
            }
            hits.record_interaction(
                layer,
                tracks.disclosure,
                tracks.disclosure,
                target,
                disclosure_spec,
            );
        }
        if let Some(target) = create_target
            && let Some(action) = create_action.as_ref()
        {
            // A filled Create keeps its own fill and its hover and pressed steps over the restored
            // row surface; a bracketed one takes the row's pointer surface on its extent.
            let painted = if action.filled(theme) {
                action.render(frame, create_pointer, interaction, theme);
                true
            } else {
                paint_pointer_target(
                    frame,
                    action.rect,
                    ControlSurface::Chrome,
                    create_pointer,
                    Role::TextSecondary,
                    Availability::Enabled,
                    theme,
                )
                .is_some()
            };
            if painted && let Some(hits) = hits.as_deref_mut() {
                hits.record_interaction(layer, action.rect, action.rect, target, auxiliary_spec);
            }
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
            // The binding closes only the current Workspace, so only its close shows the key.
            hits.record_interaction_tooltip(
                layer,
                rect,
                &target,
                TooltipContent::Action {
                    label: crate::action_menu::MenuAction::CloseWorkspace
                        .title(locale)
                        .to_owned(),
                    key: current_row(view, row)
                        .then(|| {
                            tooltip::action_key(view, ClientAction::RequestWorkspaceClose, theme)
                        })
                        .flatten(),
                },
            );
            hits.record_interaction(layer, rect, rect, target, close_spec);
        }
        // Every Agent's glyph shows on a folded Workspace's status lines, most urgent first. Each
        // takes the press fill on its own cell, and under the pointer names only its own Agent.
        let working_frame = view.machines.working_frame();
        for ((cell, agent, drawn), pointer) in status_glyphs.iter().zip(&status_pointers) {
            let state = agent.display_state();
            let surface = if *pointer == PointerPresentation::Pressed {
                Some(Role::SurfacePressed)
            } else {
                interaction
            };
            frame.render_widget(
                Paragraph::new(sidebar_text::state_glyph(state, theme, working_frame)).style(
                    theme.content_on_surface(
                        Role::SurfaceChrome,
                        surface,
                        Some(Theme::agent_role(state)),
                        theme.agent_style(state, working_frame),
                    ),
                ),
                *cell,
            );
            if let (Some(drawn), Some(hits)) = (drawn, hits.as_deref_mut()) {
                if let Some(content) = tooltip::roster(
                    [(state, agent_tokens::identity_label(agent, theme))],
                    working_frame,
                    locale,
                    theme,
                ) {
                    hits.record_interaction_tooltip(layer, *cell, drawn, content);
                }
                hits.record_interaction(layer, *cell, *cell, drawn.clone(), status_spec);
            }
        }
        if painted.is_some()
            && let (Some(hits), Some(target)) = (hits.as_deref_mut(), row_target)
        {
            // Each glyph names itself only under the pointer: an Agent's state glyph its state, a
            // folded Workspace's status glyphs their own Agents, above, and the unread dot unread
            // activity. The rest of the row gives only the name it cuts, in full.
            // While Orchbee answers this Agent's confirmations, the one it left says why.
            let waiting = match row {
                SidebarRow::Agent { row, .. } => row.sidebar_agent().and_then(|agent| {
                    native?.confirmation_wait(&agent.target.member, &agent.target.terminal)
                }),
                SidebarRow::Context { .. }
                | SidebarRow::Project { .. }
                | SidebarRow::Recents { .. } => None,
            };
            let parts = [
                agent_state.map(|state| {
                    (
                        tracks.first_line(tracks.state),
                        match waiting {
                            Some(wait) => Some(tooltip::waiting(state, wait, locale, theme)),
                            None => tooltip::describe(
                                None,
                                tooltip::legend([state], false, working_frame, locale, theme),
                            ),
                        },
                    )
                }),
                unread_marker.then(|| {
                    (
                        tracks.first_line(tracks.unread),
                        tooltip::describe(
                            None,
                            tooltip::legend([], true, working_frame, locale, theme),
                        ),
                    )
                }),
            ];
            for (rect, content) in parts.into_iter().flatten() {
                if let Some(content) = content {
                    hits.record_part_tooltip(layer, rect, hit_band, &target, content);
                }
            }
            if let Some((rect, directory)) = folder
                && let Some(content) = tooltip::describe(Some(directory), Vec::new())
            {
                hits.record_part_tooltip(layer, rect, hit_band, &target, content);
            }
            for (rect, name) in line_names {
                if let Some(content) = tooltip::describe(Some(name), Vec::new()) {
                    hits.record_part_tooltip(layer, rect, hit_band, &target, content);
                }
            }
            if let Some(content) = tooltip::describe(cut_name, Vec::new()) {
                hits.record_interaction_tooltip(layer, hit_band, &target, content);
            }
            hits.record_interaction(layer, hit_band, hit_band, target, row_spec);
        }
        y = hit_band.bottom();
    }

    if let Some(hits) = hits.as_deref_mut() {
        hits.record(
            layer,
            Rect::new(body.x, y, body.width, body.bottom().saturating_sub(y)),
            crate::tui_hit_map::HitTarget::MachineWorkspaceSurface(window),
        );
    }
    // A Project dropped below the last row moves to the end: the free rows under the list, which
    // only a window that reaches its last row has. A Workspace has no destination there.
    let footer = if window.end() >= window.total {
        Rect::new(area.x, y, area.width, area.bottom().saturating_sub(y))
    } else {
        Rect::new(area.x, area.bottom(), area.width, 0)
    };
    if insertion.is_none()
        && let Some(hits) = hits.as_deref_mut()
    {
        hits.record_sidebar_reorder_paint(crate::tui_hit_map::SidebarReorderPaint {
            viewport: frame.area(),
            layer,
            body,
            footer,
            window,
            source: source.clone(),
        });
    }
    render_scrollbar(
        frame,
        Rect::new(area.right(), area.y, trailing_padding.min(1), area.height),
        window,
        owns_keyboard,
        theme,
        layer,
        hits,
    );
}

/// The Workspace whose Agent row opens `window`, as an index into the measured entries.
fn sidebar_header(
    rows: &[SidebarRow<'_>],
    insertion: Option<usize>,
    window: crate::machine_picker::SidebarWindow,
) -> Option<usize> {
    let top = window.top;
    if insertion == Some(top) || top >= window.total {
        return None;
    }
    let row = top - usize::from(insertion.is_some_and(|slot| top > slot));
    if !matches!(rows.get(row)?, SidebarRow::Agent { .. }) {
        return None;
    }
    let owner = rows[..row]
        .iter()
        .rposition(|row| !matches!(row, SidebarRow::Agent { .. }))?;
    matches!(rows[owner], SidebarRow::Context { .. })
        .then(|| owner + usize::from(insertion.is_some_and(|slot| owner >= slot)))
}

/// The window the list paints, and the entry pinned above it: while the window opens inside a
/// Workspace's Agent rows, that Workspace's row stays at the top of the list, as the GUI's sticky
/// heading does, so the list always names whose Agents it shows. The header's lines, with the gap
/// before the first shown Agent, come out of the window, which is measured again in the lines left;
/// a list too low to keep more than half its lines for the window pins nothing.
fn sticky_window(
    rows: &[SidebarRow<'_>],
    heights: &[u16],
    gaps: &[u16],
    insertion: Option<usize>,
    cells: u16,
    window_in: impl Fn(u16) -> crate::machine_picker::SidebarWindow,
) -> (crate::machine_picker::SidebarWindow, Option<usize>) {
    let full = window_in(cells);
    let mut window = full;
    // A window measured under one Workspace's header can open on another's Agents, which measures
    // once more under that one's.
    for _ in 0..3 {
        let Some(header) = sidebar_header(rows, insertion, window) else {
            break;
        };
        let taken = heights[header].saturating_add(gaps[window.top - 1]);
        if taken.saturating_mul(2) >= cells {
            break;
        }
        let under = window_in(cells - taken);
        if sidebar_header(rows, insertion, under) == Some(header)
            && heights[header].saturating_add(gaps[under.top - 1]) == taken
        {
            return (under, Some(header));
        }
        window = under;
    }
    (full, None)
}

/// While the list overflows, the first trailing padding cell of each list line carries the
/// Client's scrollbar, the same one a pane shows: its thumb places the window among the list's
/// rows, and a press or drag there scrolls the list. A list that fits leaves the column to the row
/// surfaces and publishes no scrollbar target.
fn render_scrollbar(
    frame: &mut Frame<'_>,
    track: Rect,
    window: crate::machine_picker::SidebarWindow,
    owns_keyboard: bool,
    theme: &Theme,
    layer: HitLayer,
    hits: Option<&mut HitMapBuilder>,
) {
    let Some(range) = (|| {
        crate::terminal_scrollbar::ScrollbarRange::new(
            u32::try_from(window.total.saturating_sub(window.maximum)).ok()?,
            u32::try_from(window.maximum).ok()?,
            u32::try_from(window.top.min(window.maximum)).ok()?,
        )
    })() else {
        return;
    };
    if frame.area().intersection(track) != track || range.thumb(track).is_none() {
        return;
    }
    let _ = super::scrollbar::paint(
        frame,
        track,
        Role::SurfaceChrome,
        Some(range),
        owns_keyboard,
        theme,
    );
    if let Some(hits) = hits {
        hits.record(
            layer,
            track,
            crate::tui_hit_map::HitTarget::SidebarScrollbar {
                track,
                range,
                window,
            },
        );
    }
}
#[cfg(test)]
mod tests;
