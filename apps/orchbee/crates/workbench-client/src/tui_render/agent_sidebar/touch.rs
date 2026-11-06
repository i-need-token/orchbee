//! The mobile list spaced for touch: every fixed action keeps one line of content and owns the
//! blank line under it, New workspace and the toggle also own the window's first line, and the
//! toggle and Help take their touch size.
//!
//! Lines from the top: blank, New workspace and the toggle, blank, Search, blank, blank, then the
//! list; Settings and Help take the line above the last, which their targets share.

use super::super::sidebar_chrome::{self, TRAILING_GAP, TouchRow};
use super::super::sidebar_touch::Halos;
use super::{
    ActionKind, ActionSpec, Availability, CheckedState, ControlSurface, CurrentState, FocusState,
    InteractionState, NavigationState, OperationState, PointerPresentation, Rect, Role,
    SidebarArea, SidebarInput, Theme, action_rect, display_width, pointer_presentation,
    render_action,
};
use super::{ClientView, Frame, HitLayer, HitMapBuilder, Locale, Paragraph};
use crate::tui_hit_map::{InteractionSpec, InteractionTarget, PrimaryContract};

/// The first fixed action's content line; the line above it is the window's first.
const FIRST_ACTION_LINE: u16 = 1;
/// The second fixed action's content line, after the first action's blank line.
const SECOND_ACTION_LINE: u16 = 3;
/// The list starts after the second action's blank line and one more, as unspaced rows keep one
/// blank line after the actions.
const LIST_LINE: u16 = 6;

pub(super) struct Context<'a> {
    pub(super) native: Option<&'a crate::workbench_client::WorkbenchClientView<'a>>,
    pub(super) view: &'a ClientView,
    pub(super) locale: Locale,
    pub(super) theme: &'a Theme,
    pub(super) layer: HitLayer,
    pub(super) input: SidebarInput,
    pub(super) owns_keyboard: bool,
    pub(super) source: &'a crate::machine_picker::SidebarSource,
    pub(super) controls: &'a sidebar_chrome::Context<'a>,
    pub(super) creation:
        Option<crate::workbench_state::workspace_setup::WorkspaceCreationPreview<'a>>,
    pub(super) held: Option<&'a crate::machine_picker::SidebarTarget>,
    pub(super) sidebar_width: u16,
    pub(super) trailing_padding: u16,
}

/// One fixed action at touch size: `[ Label ]` on `place.row`, owning every line of `place` from
/// the row's start to the column its owner passes when painting it. Hover and press fill the
/// whole target on the content row, key included.
struct TouchAction<'a> {
    label: &'a str,
    kind: ActionKind,
    enabled: bool,
    target: InteractionTarget,
    place: TouchRow,
}

struct Painted {
    action: Rect,
    fill: Option<Role>,
}

fn rest_state(enabled: bool) -> InteractionState {
    InteractionState::new(
        FocusState::Unfocused,
        PointerPresentation::Rest,
        NavigationState::None,
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        if enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        OperationState::Idle,
    )
}

/// Resolves the action's painted extent before anything paints, so its trailing icon action can
/// decide whether it fits beside it.
fn resolve(frame: &Frame<'_>, action: &TouchAction<'_>, theme: &Theme) -> Option<Rect> {
    let rest = ActionSpec::new(
        action.label,
        action.kind,
        ControlSurface::Chrome,
        rest_state(action.enabled),
    );
    action_rect(frame.area(), action.place.row, rest, theme)
}

#[allow(clippy::too_many_arguments)]
fn paint_action(
    frame: &mut Frame<'_>,
    action: TouchAction<'_>,
    end: u16,
    theme: &Theme,
    layer: HitLayer,
    halos: &mut Halos,
    hits: Option<&mut HitMapBuilder>,
) -> Option<Painted> {
    let TouchAction {
        label,
        kind,
        enabled,
        target,
        place,
    } = action;
    let row = place.row;
    let hit = Rect::new(row.x, place.top, end.saturating_sub(row.x), place.height);
    if hit.is_empty() || frame.area().intersection(hit) != hit {
        return None;
    }
    let interaction = if enabled {
        InteractionSpec::enabled(PrimaryContract::ActivateOnRelease)
    } else {
        InteractionSpec::disabled(PrimaryContract::ActivateOnRelease)
    };
    let pointer = hits.as_deref().map_or(PointerPresentation::Rest, |hits| {
        pointer_presentation(hits.projected_pointer(layer, hit, hit, &target, interaction))
    });
    let pointer = if enabled {
        pointer
    } else {
        PointerPresentation::Rest
    };
    let fill = match pointer {
        PointerPresentation::Rest => None,
        PointerPresentation::Hovered => Some(Role::SurfaceHover),
        PointerPresentation::Pressed => Some(Role::SurfacePressed),
    };
    let band = Rect::new(hit.x, row.y, hit.width, 1);
    if let Some(role) = fill {
        frame.buffer_mut().set_style(
            band,
            theme.content_on_surface(
                Role::SurfaceChrome,
                Some(role),
                None,
                theme.style(Role::SurfaceChrome),
            ),
        );
    }
    halos.fill(theme, band, fill);
    let painted = render_action(
        frame,
        row,
        ActionSpec::new(
            label,
            kind,
            ControlSurface::Chrome,
            rest_state(enabled).with_pointer(pointer),
        ),
        theme,
    )?;
    if let Some(hits) = hits {
        hits.record_interaction(layer, hit, hit, target, interaction);
    }
    Some(Painted {
        action: painted,
        fill,
    })
}

/// Right-aligns `hint` to the exclusive column `right` on the action's row, two blank cells clear
/// of it, over whatever fill the action's target shows.
fn paint_hint(
    frame: &mut Frame<'_>,
    painted: &Painted,
    right: u16,
    hint: &str,
    enabled: bool,
    theme: &Theme,
) {
    let width = u16::try_from(display_width(hint)).unwrap_or(u16::MAX);
    let Some(x) = right.checked_sub(width) else {
        return;
    };
    if width == 0 || x < painted.action.right().saturating_add(TRAILING_GAP) {
        return;
    }
    let role = if enabled {
        Role::HintKey
    } else {
        Role::TextDisabled
    };
    frame.render_widget(
        Paragraph::new(hint).style(theme.content_on_surface(
            Role::SurfaceChrome,
            painted.fill,
            Some(role),
            theme.style(role),
        )),
        Rect::new(x, painted.action.y, width, 1),
    );
}

pub(super) fn render(
    frame: &mut Frame<'_>,
    sidebar: SidebarArea,
    inner: Rect,
    context: Context<'_>,
    mut hits: Option<&mut HitMapBuilder>,
) {
    use crate::machine_picker::{SidebarCommand, SidebarFixedAction};
    let Context {
        native,
        view,
        locale,
        theme,
        layer,
        input,
        owns_keyboard,
        source,
        controls,
        creation,
        held,
        sidebar_width,
        trailing_padding,
    } = context;
    let mut halos = Halos::new(sidebar.area);
    let top = sidebar.area.y;
    // Targets run through the trailing padding to the window's edge.
    let edge = inner.right().saturating_add(trailing_padding);
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
    .filter(|(command, _, _)| {
        *command != SidebarCommand::NewWorkspace
            || view
                .capabilities()
                .supports(crate::tui_capabilities::Capability::DirectoryInput)
    });
    for (slot, (command, fixed, kind)) in actions.enumerate() {
        // The first action also owns the window's first line, so the top edge has no dead line.
        let (line, place_top, height) = if slot == 0 {
            (FIRST_ACTION_LINE, 0, 3)
        } else {
            (SECOND_ACTION_LINE, SECOND_ACTION_LINE, 2)
        };
        let row = Rect::new(inner.x, top + line, inner.width, 1);
        let place = TouchRow {
            row,
            top: top + place_top,
            height,
        };
        let enabled = view.machines.available;
        let action = TouchAction {
            label: locale.sidebar_navigation_action(command),
            kind,
            enabled,
            target: InteractionTarget::SidebarFixed(
                view.machines.drawn_sidebar_fixed(source, fixed),
            ),
            place,
        };
        let resolved = resolve(frame, &action, theme);
        // The first action shares its row with the toggle, as Settings shares its row with Help.
        let toggle_x = (slot == 0)
            .then(|| sidebar_chrome::touch_trailing_x(row, resolved))
            .flatten();
        let end = toggle_x.map_or(edge, |x| x - 1);
        let painted = paint_action(
            frame,
            action,
            end,
            theme,
            layer,
            &mut halos,
            hits.as_deref_mut(),
        );
        halos.text(row);
        let toggle = toggle_x.and_then(|_| {
            sidebar_chrome::render_touch_sidebar_toggle(
                frame,
                place,
                resolved,
                controls,
                &mut halos,
                hits.as_deref_mut(),
            )
        });
        let hint = owns_keyboard
            .then(|| {
                crate::machine_picker::sidebar_binding_label(command, theme.glyphs.key_labels())
            })
            .flatten();
        if let (Some(painted), Some(hint)) = (painted.as_ref(), hint.as_deref()) {
            let right = toggle.map_or(inner.right(), |toggle| {
                toggle.x.saturating_sub(TRAILING_GAP)
            });
            paint_hint(frame, painted, right, hint, enabled, theme);
        }
    }

    // Settings and Help take the line above the last, and their targets take the last line too.
    let settings_line = sidebar.settings_row();
    let settings_row = Rect::new(inner.x, settings_line, inner.width, 1);
    let settings_place = TouchRow {
        row: settings_row,
        top: settings_line,
        height: sidebar.area.bottom().saturating_sub(settings_line),
    };
    let settings = TouchAction {
        label: locale.settings_title(),
        kind: ActionKind::Secondary,
        enabled: true,
        target: InteractionTarget::SidebarFixed(
            view.machines
                .drawn_sidebar_fixed(source, SidebarFixedAction::Settings),
        ),
        place: settings_place,
    };
    let resolved = resolve(frame, &settings, theme);
    let help_x = resolved.and_then(|_| sidebar_chrome::touch_trailing_x(settings_row, resolved));
    let painted = paint_action(
        frame,
        settings,
        help_x.map_or(edge, |x| x - 1),
        theme,
        layer,
        &mut halos,
        hits.as_deref_mut(),
    );
    halos.text(settings_row);
    if help_x.is_some() {
        let _ = sidebar_chrome::render_touch_help(
            frame,
            settings_place,
            painted.as_ref().map(|painted| painted.action),
            controls,
            &mut halos,
            hits.as_deref_mut(),
        );
    }

    // The list runs from under the actions down to the blank line above Settings.
    let list_top = top + LIST_LINE;
    let mut area = Rect::new(
        inner.x,
        list_top,
        inner.width,
        settings_line.saturating_sub(list_top),
    );
    if view.onboarding.visible
        && input == SidebarInput::Terminal
        && !view.prefix_active()
        && !view.navigator_active()
        && !view.resize_pending
        && let Some(guidance) = super::super::onboarding::area(area)
        && let Some(hits) = hits.as_deref_mut()
    {
        super::super::onboarding::render(frame, guidance, view, locale, layer, theme, hits);
        halos.text(guidance);
        area.height = area.height.saturating_sub(guidance.height);
    }
    let rows = view.machines.sidebar_navigation_rows();
    super::super::workspace_sidebar::render(
        frame,
        area,
        super::super::workspace_sidebar::RenderContext {
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
        },
        &rows,
        Some(&mut halos),
        hits,
    );
    halos.paint(frame, theme);
}
