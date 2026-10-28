//! Paints the existing merged Agent projection; never reads or acknowledges terminals.
use super::controls::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, action_rect,
    render_action,
};
use super::sidebar_text::machine_label;
use super::{
    Block, Borders, Clear, ClientView, Frame, HitLayer, HitMapBuilder, Line, Locale, Padding,
    Paragraph, Rect, Role, SessionStatus, SidebarArea, SidebarLayer, Span, Theme, display_width,
};
use crate::client_view::{KeyLabelGlyphs, Overlay};
use crate::machine_discovery::display_label;
use crate::tui_layout::{DrawnSidebarWidth, SidebarWidthDrag};
use crate::tui_pointer::{
    HitTarget, InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer,
};
use ratatui::style::Style;
pub(super) mod tokens;

pub(super) fn key_label_glyphs(theme: &Theme) -> KeyLabelGlyphs {
    match theme.glyphs.mode() {
        crate::tui_theme::GlyphMode::Unicode => KeyLabelGlyphs::Unicode,
        #[cfg(any(test, feature = "test-support"))]
        crate::tui_theme::GlyphMode::Simplified => KeyLabelGlyphs::Ascii,
    }
}

fn pointer_presentation(pointer: ProjectedPointer) -> PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => PointerPresentation::Rest,
        ProjectedPointer::Hovered => PointerPresentation::Hovered,
        ProjectedPointer::Pressed => PointerPresentation::Pressed,
    }
}

struct FixedAction<'a> {
    label: &'a str,
    hint: Option<&'a str>,
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
    pub(super) view: &'a ClientView,
    pub(super) sidebar_surface: crate::client_view::SidebarSurface,
    pub(super) locale: Locale,
    pub(super) backend_status: SessionStatus<'a>,
    pub(super) theme: &'a Theme,
    pub(super) input: SidebarInput,
    pub(super) creation: Option<crate::workbench_client::WorkspaceCreationPreview<'a>>,
}

struct ActionsContext<'a> {
    view: &'a ClientView,
    locale: Locale,
    theme: &'a Theme,
    layer: HitLayer,
    owns_keyboard: bool,
    source: &'a crate::machine_picker::SidebarSource,
    hint_right: u16,
}

/// `hint_right` is the exclusive column where a shortcut hint ends; it may pass `area` into the
/// sidebar's trailing padding, up to its divider.
fn render_fixed_action(
    frame: &mut Frame<'_>,
    area: Rect,
    hint_right: u16,
    action: FixedAction<'_>,
    theme: &Theme,
    layer: HitLayer,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Rect> {
    let FixedAction {
        label,
        hint,
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
    if let Some(hint) = hint {
        let hint_width = u16::try_from(display_width(hint)).unwrap_or(u16::MAX);
        if let Some(hint_x) = hint_right.checked_sub(hint_width)
            && hint_width > 0
            && hint_x >= rect.right().saturating_add(2)
        {
            frame.render_widget(
                Paragraph::new(hint).style(theme.style(if enabled {
                    Role::HintKey
                } else {
                    Role::TextDisabled
                })),
                Rect::new(hint_x, area.y, hint_width, 1),
            );
        }
    }
    Some(painted)
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
    } = context;
    for (offset, (command, action, kind)) in [
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
    .filter(|(command, _, _)| {
        *command != SidebarCommand::NewWorkspace
            || view
                .capabilities
                .supports(crate::tui_capabilities::Capability::DirectoryInput)
    })
    .enumerate()
    {
        if offset >= usize::from(area.height) {
            break;
        }
        let row = Rect::new(area.x, area.y + offset as u16, area.width, 1);
        let hint = owns_keyboard
            .then(|| crate::machine_picker::sidebar_binding_label(command, key_label_glyphs(theme)))
            .flatten();
        let target =
            InteractionTarget::SidebarFixed(view.machines.drawn_sidebar_fixed(source, action));
        let _ = render_fixed_action(
            frame,
            row,
            hint_right,
            FixedAction {
                label: locale.sidebar_navigation_action(command),
                hint: hint.as_deref(),
                kind,
                enabled: view.machines.available,
                target,
            },
            theme,
            layer,
            hits.as_deref_mut(),
        );
    }
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    context: RenderContext<'_>,
    mut hits: Option<&mut HitMapBuilder>,
) {
    let RenderContext {
        view,
        sidebar_surface,
        locale,
        backend_status: _backend_status,
        theme,
        input,
        creation,
    } = context;
    let owns_keyboard = input == SidebarInput::Navigation;
    if sidebar.layer == SidebarLayer::Overlay {
        frame.render_widget(Clear, sidebar.area);
    }
    let block = Block::new()
        .borders(Borders::RIGHT)
        .border_style(theme.style(if owns_keyboard {
            Role::AccentFocus
        } else {
            Role::FrameDivider
        }))
        .padding(Padding::new(0, 1, 1, 0))
        .style(theme.style(Role::SurfaceChrome));
    let mut inner = block.inner(sidebar.area);
    // The fixed actions continue through the trailing padding to the divider.
    let content_right = sidebar.area.right().saturating_sub(1).max(inner.right());
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
    let source = view.machines.sidebar_source();
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
        },
        hits.as_deref_mut(),
    );
    let action_rows = inner
        .height
        .min(if frame.area().height >= 24 { 3 } else { 2 });
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
    let _ =
        render_fixed_action(
            frame,
            settings,
            content_right,
            FixedAction {
                label: locale.settings_title(),
                hint: None,
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
    area.height = area.height.saturating_sub(footer_rows);
    if view.onboarding.visible
        && input == SidebarInput::Terminal
        && !view.prefix_active()
        && !view.navigator_active()
        && !view.resize_pending
        && let Some(guidance) = super::onboarding::area(area)
        && let Some(hits) = hits.as_deref_mut()
    {
        super::onboarding::render(frame, guidance, view, locale, layer, theme, hits);
        area.height = area.height.saturating_sub(guidance.height);
    }
    super::workspace_sidebar::render(
        frame,
        area,
        super::workspace_sidebar::RenderContext {
            view,
            locale,
            theme,
            layer,
            sidebar_width: sidebar.area.width,
            trailing_padding: content_right - area.right(),
            owns_keyboard,
            source: &source,
            creation,
        },
        &rows,
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
        || view.resize_mode()
        || view.machines.open
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
