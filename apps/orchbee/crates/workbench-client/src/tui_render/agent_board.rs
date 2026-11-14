//! The Start Agent picker overlay and the shared control helpers its rows use. The picker
//! records its hit regions in the same pass, so pointer presses resolve against the exact draw.

use ratatui::{Frame, layout::Rect};

use crate::agent_board::{AgentBoardControl, AgentBoardDraw, AgentBoardView};
use crate::localization::ShellText;
use crate::tui_hit_map::{
    HitLayer, HitMapBuilder, InteractionSpec, InteractionTarget, PrimaryContract, ProjectedPointer,
};
use crate::tui_theme::Theme;

mod picker;

/// Paints the open picker and records its hit regions in the same pass. Geometry refusal carries
/// the exact open episode; success carries the owner's complete visible-window witness.
pub(super) fn render(
    frame: &mut Frame<'_>,
    text: ShellText,
    board: &AgentBoardView<'_>,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> AgentBoardDraw {
    picker::render(frame, text, board, theme, hits)
}

fn pointer_presentation(pointer: ProjectedPointer) -> super::controls::PointerPresentation {
    match pointer {
        ProjectedPointer::Rest => super::controls::PointerPresentation::Rest,
        ProjectedPointer::Hovered => super::controls::PointerPresentation::Hovered,
        ProjectedPointer::Pressed => super::controls::PointerPresentation::Pressed,
    }
}

fn interaction_state(
    enabled: bool,
    selected: bool,
    pending: bool,
    pointer: super::controls::PointerPresentation,
) -> super::controls::InteractionState {
    use super::controls::{
        Availability, CheckedState, CurrentState, FocusState, NavigationState, OperationState,
    };
    super::controls::InteractionState::new(
        FocusState::Unfocused,
        pointer,
        if selected {
            NavigationState::Provisional
        } else {
            NavigationState::None
        },
        CurrentState::NotCurrent,
        CheckedState::Unchecked,
        if enabled {
            Availability::Enabled
        } else {
            Availability::Disabled
        },
        if pending {
            OperationState::Pending
        } else {
            OperationState::Idle
        },
    )
}

fn render_header_close(
    frame: &mut Frame<'_>,
    area: Rect,
    draw: &crate::agent_board::AgentBoardActualDraw,
    theme: &Theme,
    hits: &mut HitMapBuilder,
) -> Option<Rect> {
    use super::controls::{Availability, ControlSurface, IconActionGlyph, IconActionSpec};
    let target = draw.control_target(AgentBoardControl::Close);
    let interaction = InteractionTarget::AgentBoardControl(target);
    let spec = InteractionSpec::enabled(PrimaryContract::ActivateOnRelease);
    let rect = super::controls::frame_close_rect(frame.area(), area)?;
    let pointer = pointer_presentation(hits.projected_pointer(
        HitLayer::Overlay,
        rect,
        rect,
        &interaction,
        spec,
    ));
    let painted = super::controls::render_frame_icon_action(
        frame,
        rect,
        IconActionSpec::new(
            IconActionGlyph::Close,
            ControlSurface::Overlay,
            Availability::Enabled,
            pointer,
        ),
        theme,
    )?;
    if painted != rect {
        return None;
    }
    hits.record_interaction(HitLayer::Overlay, painted, painted, interaction, spec);
    Some(painted)
}
