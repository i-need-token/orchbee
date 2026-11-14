//! Paints the existing merged Agent projection; never reads or acknowledges terminals.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, action_rect,
    render_action,
};
use super::{
    Block, Borders, Clear, ClientView, Frame, HitLayer, HitMapBuilder, Locale, Padding, Paragraph,
    Rect, Role, SessionStatus, SidebarArea, SidebarLayer, Theme, display_width,
};
use crate::client_view::Overlay;
use crate::tui_hit_map::{
    HitTarget, InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer,
};
use crate::tui_layout::{DrawnSidebarWidth, SidebarWidthDrag};
use ratatui::style::Style;

mod touch;

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

struct FixedAction<'a> {
    label: &'a str,
    kind: ActionKind,
    enabled: bool,
    target: InteractionTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SidebarInput {
    Suppressed,
    Terminal,
    Navigation,
}

pub(super) struct RenderContext<'a> {
    pub(super) native: Option<&'a crate::workbench_client::WorkbenchClientView<'a>>,
    pub(super) view: &'a ClientView,
    pub(super) sidebar_surface: crate::client_view::SidebarSurface,
    pub(super) locale: Locale,
    pub(super) backend_status: SessionStatus<'a>,
    pub(super) theme: &'a Theme,
    pub(super) input: SidebarInput,
    pub(super) creation:
        Option<crate::workbench_state::workspace_setup::WorkspaceCreationPreview<'a>>,
    /// The row whose pointer-opened menu is open keeps its hover presentation.
    pub(super) held: Option<&'a crate::machine_picker::SidebarTarget>,
}

struct ActionsContext<'a> {
    view: &'a ClientView,
    locale: Locale,
    theme: &'a Theme,
    layer: HitLayer,
    owns_keyboard: bool,
    source: &'a crate::machine_picker::SidebarSource,
    hint_right: u16,
    controls: &'a super::sidebar_chrome::Context<'a>,
}

fn render_fixed_action(
    frame: &mut Frame<'_>,
    area: Rect,
    action: FixedAction<'_>,
    theme: &Theme,
    layer: HitLayer,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let FixedAction {
        label,
        kind,
        enabled,
        target,
    } = action;
    let availability = if enabled {
        Availability::Enabled
    } else {
        Availability::Disabled
    };
    let base_state = InteractionState::new(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        availability,
        OperationState::Idle,
    );
    let rest = ActionSpec::new(label, kind, ControlSurface::Chrome, base_state);
    let rect = action_rect(frame.area(), area, rest, theme)?;
    let interaction = if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    };
    let pointer = hits.as_deref().map_or(PointerPresentation::Rest, |hits| {
        pointer_presentation(hits.projected_pointer(layer, rect, rect, &target, interaction))
    });
    let painted = render_action(
        frame,
        area,
        ActionSpec::new(
            label,
            kind,
            ControlSurface::Chrome,
            base_state.with_pointer(pointer),
        ),
        theme,
    )?;
    if painted != rect {
        return None;
    }
    if let Some(hits) = hits {
        hits.record_interaction(layer, rect, rect, target, interaction);
    }
    Some(painted)
}

/// Right-aligns `hint` to `hint_right`, the exclusive column where it ends, on the row of the
/// painted `action`. It may pass the list into the sidebar's trailing padding, up to its divider,
/// and drops when it cannot keep two blank cells after the action.
fn render_fixed_hint(
    frame: &mut Frame<'_>,
    action: Rect,
    hint_right: u16,
    hint: &str,
    enabled: bool,
    theme: &Theme,
) {
    let hint_width = u16::try_from(display_width(hint)).unwrap_or(u16::MAX);
    if let Some(hint_x) = hint_right.checked_sub(hint_width)
        && hint_width > 0
        && hint_x >= action.right().saturating_add(2)
    {
        frame.render_widget(
            Paragraph::new(hint).style(theme.style(if enabled {
                Role::HintKey
            } else {
                Role::TextDisabled
            })),
            Rect::new(hint_x, action.y, hint_width, 1),
        );
    }
}

fn render_actions(
    frame: &mut Frame<'_>,
    area: Rect,
    context: ActionsContext<'_>,
    mut hits: Option<&mut HitMapBuilder>,
) {
    use crate::machine_picker::{SidebarCommand, SidebarFixedAction};
    let ActionsContext {
        view,
        locale,
        theme,
        layer,
        owns_keyboard,
        source,
        hint_right,
        controls,
    } = context;
    let actions = [
        (
            SidebarCommand::NewWorkspace,
            SidebarFixedAction::NewWorkspace,
            ActionKind::Primary,
        ),
        (
            SidebarCommand::Search,
            SidebarFixedAction::Search,
            ActionKind::Secondary,
        ),
    ]
    .into_iter()
    .filter(|(command, _, _)| match command {
        SidebarCommand::NewWorkspace => view
            .capabilities()
            .supports(crate::tui_capabilities::Capability::DirectoryInput),
        SidebarCommand::Search => view.capabilities().offers_search(),
        _ => true,
    })
    .collect::<Vec<_>>();
    // Without a fixed action the toggle keeps the first row on its own.
    if actions.is_empty() && area.height > 0 {
        let row = Rect::new(area.x, area.y, area.width, 1);
        super::sidebar_chrome::render_sidebar_toggle(frame, row, None, controls, hits);
        return;
    }
    for (offset, (command, action, kind)) in actions.into_iter().enumerate() {
        if offset >= usize::from(area.height) {
            break;
        }
        let row = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        let hint = owns_keyboard
            .then(|| {
                crate::machine_picker::sidebar_binding_label(command, theme.glyphs.key_labels())
            })
            .flatten();
        let target =
            InteractionTarget::SidebarFixed(view.machines.drawn_sidebar_fixed(source, action));
        let enabled = view.machines.available;
        let painted = render_fixed_action(
            frame,
            row,
            FixedAction {
                label: locale.sidebar_navigation_action(command),
                kind,
                enabled,
                target,
            },
            theme,
            layer,
            hits.as_deref_mut(),
        );
        // The first action shares its row with the hide toggle, as Settings shares its row with
        // Help, so the toggle stays on the first row whether the sidebar is shown or hidden.
        let toggle = if offset == 0 {
            super::sidebar_chrome::render_sidebar_toggle(
                frame,
                row,
                painted,
                controls,
                hits.as_deref_mut(),
            )
        } else {
            None
        };
        if let (Some(painted), Some(hint)) = (painted, hint.as_deref()) {
            let hint_right = toggle.map_or(hint_right, |toggle| {
                toggle.x.saturating_sub(super::sidebar_chrome::TRAILING_GAP)
            });
            render_fixed_hint(frame, painted, hint_right, hint, enabled, theme);
        }
    }
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    context: RenderContext<'_>,
    mut hits: Option<&mut HitMapBuilder>,
) {
    let RenderContext {
        native,
        view,
        sidebar_surface,
        locale,
        backend_status: _backend_status,
        theme,
        input,
        creation,
        held,
    } = context;
    let owns_keyboard = input == SidebarInput::Navigation;
    if sidebar.covers() {
        frame.render_widget(Clear, sidebar.area);
    }
    // The mobile list fills the window: it keeps the trailing padding but has no divider beside
    // any content.
    let screen = sidebar.layer == SidebarLayer::Screen;
    let block = Block::new()
        .borders(if screen {
            Borders::NONE
        } else {
            Borders::RIGHT
        })
        .border_style(theme.style(if owns_keyboard {
            Role::AccentFocus
        } else {
            Role::FrameDivider
        }))
        .padding(Padding::new(0, 1, 0, 0))
        .style(theme.style(Role::SurfaceChrome));
    let mut inner = block.inner(sidebar.area);
    // The fixed actions continue through the trailing padding to the divider, which for the
    // mobile list lies just past the window's edge.
    let content_right = if screen {
        sidebar.area.right()
    } else {
        sidebar.area.right().saturating_sub(1).max(inner.right())
    };
    // Row density follows the width the list would have with its divider.
    let sidebar_width = sidebar.area.width.saturating_add(u16::from(screen));
    frame.render_widget(block, sidebar.area);
    let layer = if view.overlay() == Some(Overlay::BusinessSidebar) {
        HitLayer::Overlay
    } else {
        HitLayer::Chrome
    };
    if let Some(target) = DrawnSidebarWidth::new(frame.area(), sidebar_surface)
        && target.sidebar == sidebar
        && let Some(hits) = hits.as_deref_mut()
    {
        hits.record(layer, target.edge(), HitTarget::SidebarResize(target));
    }
    if layer == HitLayer::Chrome
        && let Some(hits) = hits.as_deref_mut()
    {
        hits.record(
            HitLayer::ChromeSurface,
            sidebar.area,
            HitTarget::OverlaySurface,
        );
    }
    let controls = super::sidebar_chrome::Context {
        view,
        locale,
        theme,
        layer,
    };
    let source = view.machines.sidebar_source();
    if sidebar.touch_spaced() {
        touch::render(
            frame,
            sidebar,
            inner,
            touch::Context {
                native,
                view,
                locale,
                theme,
                layer,
                owns_keyboard,
                source: &source,
                controls: &controls,
                creation,
                held,
                sidebar_width,
                trailing_padding: content_right.saturating_sub(inner.right()),
            },
            hits,
        );
        return;
    }
    render_actions(
        frame,
        inner,
        ActionsContext {
            view,
            locale,
            theme,
            layer,
            owns_keyboard,
            source: &source,
            hint_right: content_right,
            controls: &controls,
        },
        hits.as_deref_mut(),
    );
    // A withheld Search row gives its line back to the list.
    let action_rows = inner.height.min(
        if frame.area().height >= 24 { 3 } else { 2 }
            - u16::from(!view.capabilities().offers_search()),
    );
    inner.y += action_rows;
    inner.height -= action_rows;
    // Build the exact Workspace -> Agent projection once for this frame. The same
    // borrowed rows drive text, hit geometry, focus, and the unified scroll window.
    let rows = view.machines.sidebar_navigation_rows();
    let mut area = inner;
    if area.is_empty() {
        return;
    }
    // Settings sits alone on the last row; the list runs down to it. The floating status, not a
    // sidebar hint row, carries the shortcuts.
    let footer_rows = 1;
    let settings = Rect::new(area.x, area.bottom() - 1, area.width, 1);
    let settings_action =
        render_fixed_action(
            frame,
            settings,
            FixedAction {
                label: locale.settings_title(),
                kind: ActionKind::Secondary,
                enabled: true,
                target: InteractionTarget::SidebarFixed(view.machines.drawn_sidebar_fixed(
                    &source,
                    crate::machine_picker::SidebarFixedAction::Settings,
                )),
            },
            theme,
            layer,
            hits.as_deref_mut(),
        );
    let help = super::sidebar_chrome::render_help(
        frame,
        settings,
        settings_action,
        &controls,
        hits.as_deref_mut(),
    );
    // The environment marker yields first: it takes only what Settings and Help leave.
    if let (Some(action), Some(help)) = (settings_action, help) {
        let (start, end) = super::environment_marker::bounds(action.right(), help.x);
        if let Some(marker) = super::environment_marker::layout(&controls, settings.y, start, end) {
            super::environment_marker::paint(
                frame,
                marker,
                sidebar.area,
                &controls,
                hits.as_deref_mut(),
            );
        }
    }
    area.height = area.height.saturating_sub(footer_rows);
    super::workspace_sidebar::render(
        frame,
        area,
        super::workspace_sidebar::RenderContext {
            native,
            view,
            locale,
            theme,
            layer,
            sidebar_width,
            trailing_padding: content_right - area.right(),
            owns_keyboard,
            source: &source,
            creation,
            held,
        },
        &rows,
        None,
        hits,
    );
}

/// Highlight the actual projected border; layout and hit regions already use this width.
pub fn render_width_preview(
    frame: &mut Frame<'_>,
    view: &ClientView,
    drag: &SidebarWidthDrag,
    theme: &Theme,
    hits: &HitMapBuilder,
) -> bool {
    if !matches!(view.overlay(), None | Some(Overlay::BusinessSidebar))
        || view.prefix_active()
        || view.machines.captures()
        || view.pointer_released()
        || DrawnSidebarWidth::new(frame.area(), view.sidebar_surface()) != Some(drag.target)
        || !hits.exposes_sidebar_width(&drag.projected_target())
    {
        return false;
    }
    let glyph = if theme.glyphs.ellipsis == ".." {
        "|"
    } else {
        "│"
    };
    for y in drag.preview.y..drag.preview.bottom() {
        frame.buffer_mut().set_string(
            drag.preview.x,
            y,
            glyph,
            Style::new().fg(theme.color(Role::AccentFocus)),
        );
    }
    true
}

#[cfg(test)]
mod tests;
