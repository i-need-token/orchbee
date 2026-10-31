//! Pointer ownership state for the native shell.
//!
//! Three contracts live here, matching `docs/tui/mouse-input.md`:
//!
//! - **Resolution against the published hit map** that `tui_hit_map` owns:
//!   the renderer records painted regions, the shell publishes the map only
//!   after a complete successful draw, and pointer events resolve against the
//!   last published frame's regions — never a predicted layout. Regions carry
//!   stable identities, re-resolved to positions at click time via the port's
//!   `PointerProbe`.
//! - The **interaction projection** handed to each candidate frame for
//!   painted controls: hover and armed presentation stay transient, while a
//!   matching release emits only the feature-declared semantic intent.
//! - The **gesture lifecycle**: idle → pane / pending-focus, with the
//!   bounded pending queue that holds a pane press while its focus switch
//!   installs. A press and its release resolve against the press's region;
//!   a drag keeps its region until release.
//! - **Wheel normalization**: the alternate-screen fallback accumulates
//!   cursor-key presses, three per notch, flushing at most one viewport of
//!   arrows per pass so a trackpad stream cannot replay a flood.
//!
//! The dispatcher owns one `PointerState`; the shell feeds it the painted
//! map and the capture flag settled by the lifecycle layer.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use ratatui::layout::Rect;

use orchbee_terminal_protocol::MouseTracking;

pub use crate::client_view::help::HelpControl;
use crate::client_view::{ClientAction, ClientView, Modifiers};
use crate::terminal_events::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PanePointerRoute,
    encode_cursor_arrows, encode_pane_report, route_pane_pointer,
};
use crate::workbench_client::input::{
    BoardAction, CloseDecision, CopyAction, LayoutAction, MenuAction, PaneAdmission, PaneInput,
    PointerFrameFacts, PointerProbe, PointerSelectionCell, PointerSelectionIntent, RenameAction,
    SetupAction, SurfaceAction, SurfaceInput, TerminalInputPort, WorktreeAction,
};
use crate::workbench_state::PaneRef;

/// The application facade names the builder through this module.
pub use crate::tui_hit_map::HitMapBuilder;
use crate::tui_hit_map::{
    FrameHitMap, HitLayer, HitRegionResolution, HitResolution, HitTarget, InteractionTarget,
    ListScrollTarget, ModalControl, PointerProjection, PrimaryContract, ResizeControl,
    SidebarReorderDraw, SidebarReorderPaint, TabDragPreview, contains,
};

mod chrome;
mod copy_mode;
mod interaction;
pub use interaction::MotionObservability;
pub(crate) use interaction::ReleaseOwnership;
mod list_scroll;
mod modal;
pub(crate) use modal::CancelledModalEvent;
mod scrollbar;
mod sidebar_reorder;
mod tabs;
pub(crate) use sidebar_reorder::SidebarReorderPreview;
mod sidebar_width;
mod tooltip;

/// Longest a queued pane press waits for its focus switch to install.
/// Bounded so a stuck attach cannot hold input hostage.
const PENDING_FOCUS_TIMEOUT: Duration = Duration::from_secs(2);

/// Bounded pending-focus queue: presses, motions, and wheel notches that
/// arrived while the target pane's switch installed. Overflow cancels the
/// gesture before accepting more input, with a user-facing notice.
const PENDING_FOCUS_LIMIT: usize = 16;

/// Maximum interval between two presses on the exact same pane cell that
/// promotes the second press from cell selection to word selection.
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(350);

/// Cursor-key presses emitted per wheel notch under the alternate-screen
/// fallback — the pager-friendly amount the mouse standard fixes.
const ARROWS_PER_NOTCH: usize = 3;

/// The wheel-fallback accumulator: pending arrow presses in one
/// direction. A reversal replaces the pending stream with its first new
/// notch; the flush emits at most one viewport of arrows so a
/// trackpad burst stays bounded.
#[derive(Default)]
struct WheelStream {
    /// `Some(direction)` while the burst is open. A tick that finds the
    /// queue empty closes the stream, so the next notch starts fresh.
    up: Option<bool>,
    pending: usize,
    /// Arrows emitted since the last tick — the per-pass bound.
    emitted: usize,
    /// The pane viewport height — the per-pass cap.
    viewport_rows: usize,
    application_cursor: bool,
}

impl WheelStream {
    fn accumulate(&mut self, up: bool, viewport_rows: usize, application_cursor: bool) {
        if self.up == Some(!up) {
            // Retire the old direction's remainder, then admit the first
            // new notch. A direction change must not become a dead notch.
            self.pending = 0;
            self.emitted = 0;
        }
        self.pending = self.pending.saturating_add(ARROWS_PER_NOTCH);
        self.up = Some(up);
        self.viewport_rows = viewport_rows;
        self.application_cursor = application_cursor;
    }

    fn clear(&mut self) {
        self.up = None;
        self.pending = 0;
        self.emitted = 0;
    }

    /// A burst emits at most `viewport_rows` arrows per pass; the
    /// remainder stays queued for later ticks.
    fn take_flush(&mut self) -> Option<(bool, usize, bool)> {
        let up = self.up?;
        let emit = self
            .pending
            .min(self.viewport_rows.max(1).saturating_sub(self.emitted));
        if emit == 0 {
            return None;
        }
        self.pending -= emit;
        self.emitted += emit;
        Some((up, emit, self.application_cursor))
    }
}

/// The exact pane incarnation and painted content rect a press or wheel
/// event targets — carried as one value through the gesture paths. The
/// ref is the record's live identity at paint time: anchor generation,
/// stable panel, and incarnation all compare at once, so an event can
/// never land on a replacement incarnation of the same logical pane.
struct PaneTarget {
    expected: PaneRef,
    content: Rect,
}

/// A pane press waiting for its focus switch: the queued input plus the
/// press-time facts needed to adjudicate readiness against a *fresh*
/// probe — the expected exact incarnation and the painted viewport for
/// coordinate translation. The `expected` ref is compared bit for bit
/// against each probe's live facts; a re-minted record of the same
/// `(anchor, panel)` fails the equality and orphans the queue.
struct PendingFocus {
    button: MouseButton,
    expected: PaneRef,
    viewport: Rect,
    pressed_at: Instant,
    deadline: Instant,
    queued: VecDeque<MouseEvent>,
}

/// A Host-backed selection admitted for one exact pane incarnation. Its
/// press-time viewport remains the coordinate authority through repaint and
/// output revisions until End or Cancel.
struct SelectionGesture {
    expected: PaneRef,
    viewport: Rect,
}

/// Ctrl+Left owns one exact painted pane cell through its matching release.
/// Cancellation is sticky so a later release is swallowed rather than
/// leaking into a pane after map, modifier, or membership drift.
struct LinkGesture {
    source: Option<crate::terminal_link_activation::PaintedLinkSource>,
    viewport: Rect,
    press_column: u16,
    press_row: u16,
    cell: PointerSelectionCell,
    cancelled: bool,
}

impl LinkGesture {
    fn alive(&self, probe: &PointerProbe) -> bool {
        self.source.as_ref().is_some_and(|source| {
            probe
                .panes
                .iter()
                .any(|pane| pane.record == source.pane && pane.attached)
        })
    }

    fn source_matches(&self, map: &FrameHitMap) -> bool {
        self.source.as_ref().is_some_and(|source| {
            match map.hit(self.press_column, self.press_row) {
                HitResolution::Region(HitTarget::Pane { pane, content })
                    if *content == self.viewport && pane == &source.pane =>
                {
                    map.link_source(pane, self.press_column, self.press_row) == Some(source)
                }
                _ => false,
            }
        })
    }
}

impl SelectionGesture {
    fn alive(&self, probe: &PointerProbe) -> bool {
        probe.selection_ready
            && probe.focus.as_ref() == Some(&self.expected)
            && probe
                .panes
                .iter()
                .any(|pane| pane.record == self.expected && pane.attached)
    }
}

/// The last admitted selection press. Count two is terminal: a third press
/// begins a fresh single-click sequence rather than minting line selection.
struct SelectionClick {
    expected: PaneRef,
    cell: PointerSelectionCell,
    at: Instant,
    count: u8,
}

/// Forwards pointer-encoded bytes; a refused forward becomes the Client's input-unavailable notice.
fn forward_or_report(
    view: &mut ClientView,
    input: &[u8],
    terminal_input: &mut (impl TerminalInputPort + ?Sized),
) {
    if crate::terminal_input::navigation_holds_input(view, terminal_input) {
        return;
    }
    if terminal_input.forward(input).is_err() {
        view.report_terminal_input_unavailable();
    }
}

fn selection_cell(viewport: Rect, event: MouseEvent) -> Option<PointerSelectionCell> {
    if viewport.width == 0 || viewport.height == 0 {
        return None;
    }
    Some(PointerSelectionCell {
        column: event
            .column
            .saturating_sub(viewport.x)
            .min(viewport.width - 1),
        row: event
            .row
            .saturating_sub(viewport.y)
            .min(viewport.height - 1),
    })
}

fn plain_primary_press(event: MouseEvent) -> bool {
    event.action == MouseAction::Press
        && event.button == MouseButton::Left
        && event.mods == crate::client_view::Modifiers::NONE
}

fn link_press(event: MouseEvent) -> bool {
    event.action == MouseAction::Press
        && event.button == MouseButton::Left
        && event.mods == crate::client_view::Modifiers::CTRL
}

/// A live pane gesture: the exact pane incarnation that owns the pointer
/// until release, its painted viewport for local coordinates, and the
/// encoder state — held button and motion dedup cell.
struct PaneGesture {
    button: MouseButton,
    expected: PaneRef,
    viewport: Rect,
    pressed: bool,
    last_cell: Option<(u16, u16)>,
}

fn pane_input_ready(probe: &PointerProbe, expected: &PaneRef) -> bool {
    probe.input_ready
        && probe.focus.as_ref() == Some(expected)
        && probe.panes.iter().any(|pane| {
            pane.record == *expected
                && pane.attached
                && pane.controlling == Some(true)
                && pane.frame_available
        })
}

impl PaneGesture {
    /// Whether this gesture still owns the pointer: its recorded
    /// incarnation is still the live focus pane and still attached.
    /// Anything else ends the gesture — a release is never delivered to a
    /// pane that lost the anchor, and never migrates to a fresh
    /// incarnation of the same logical pane.
    fn alive(&self, probe: &PointerProbe) -> bool {
        probe.focus.as_ref() == Some(&self.expected)
            && probe
                .panes
                .iter()
                .any(|pane| pane.record == self.expected && pane.attached)
    }

    fn accepts(&self, event: MouseEvent) -> bool {
        if matches!(
            event.button,
            MouseButton::WheelUp
                | MouseButton::WheelDown
                | MouseButton::WheelLeft
                | MouseButton::WheelRight
        ) {
            return event.action == MouseAction::Press;
        }
        match event.action {
            MouseAction::Press => event.button == self.button && !self.pressed,
            MouseAction::Motion => event.button == self.button,
            MouseAction::Release => {
                event.button == self.button || event.button == MouseButton::None
            }
        }
    }

    fn forward<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        frame: &PointerFrameFacts,
        event: MouseEvent,
    ) {
        let local = PaneMouseEvent {
            action: event.action,
            button: if event.action == MouseAction::Release && event.button == MouseButton::None {
                Some(self.button)
            } else {
                (event.button != MouseButton::None).then_some(event.button)
            },
            mods: event.mods,
            column: i32::from(event.column) - i32::from(self.viewport.x),
            row: i32::from(event.row) - i32::from(self.viewport.y),
        };
        match encode_pane_report(
            &local,
            frame.tracking,
            frame.format,
            (self.viewport.width, self.viewport.height),
            self.pressed,
            &mut self.last_cell,
        ) {
            PaneEncode::Bytes(bytes) => forward_or_report(view, &bytes, port),
            PaneEncode::Silent => {}
        }
        match event.action {
            MouseAction::Press
                if !matches!(
                    event.button,
                    MouseButton::WheelUp
                        | MouseButton::WheelDown
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                ) =>
            {
                self.pressed = true;
            }
            MouseAction::Release => self.pressed = false,
            _ => {}
        }
    }
}

#[derive(Default)]
enum Gesture {
    #[default]
    Idle,
    Tab(tabs::TabDrag),
    SidebarReorder(sidebar_reorder::SidebarReorder),
    TabCancelled,
    Pending(PendingFocus),
    Pane(PaneGesture),
    Selection(SelectionGesture),
    Scrollbar(scrollbar::ScrollbarDrag),
    Divider(crate::tui_layout::DividerDrag),
    SidebarWidth(crate::tui_layout::SidebarWidthDrag),
    Link(LinkGesture),
}

/// Minimal cleanup ownership retained after an accepted Windows chrome
/// gesture loses focus, capture, or painted authority. Domain payload and
/// geometry are deliberately absent: the debt may only consume the matching
/// primary release once and can never replay the retired effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CancelledChromeRelease {
    SidebarReorder,
    Divider,
    SidebarWidth,
}

/// What one host report did to the pointer state — the dispatcher maps it
/// to `after_layout` work and map invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PointerOutcome {
    SidebarReorderEnded,
    TabGestureEnded,
    SidebarWidthEnded,
    MenuRequested {
        target: crate::action_menu::MenuTarget,
        anchor: (u16, u16),
        button: MouseButton,
    },
    /// A feature-declared generic interaction crossed its commit boundary.
    /// The dispatcher maps this typed target to the owning semantic action.
    Interaction {
        target: InteractionTarget,
        contract: PrimaryContract,
    },
    /// A previously accepted generic press ended without release activation.
    /// Its physical release packet remains Client-owned, but no target is re-hit.
    InteractionEnded,
    /// A cancelled specialized chrome press consumed its matching release.
    ChromeGestureEnded,
    /// A button-free movement changed, or did not change, transient pointer
    /// presentation without any feature or pane effect.
    IdleMotion {
        changed: bool,
    },
    /// Not ours or swallowed — the packet moves on untouched.
    Ignored,
    /// Consumed without changing structural regions: pane-bound bytes, a
    /// queued event, a wheel notch, or transient pressed presentation. The
    /// recorded map stays authoritative.
    Handled,
    /// Consumed and painted regions may differ (navigation applied, an
    /// overlay dismissed): the map is stale until the next draw. The
    /// gesture the event belongs to survives — release/motion follow it.
    RegionsChanged,
}

#[cfg(test)]
impl PointerOutcome {
    /// The event was consumed — the dispatcher runs the viewport-sync
    /// path. `Ignored` is the only outcome that skips it.
    pub(crate) fn handled(self) -> bool {
        !matches!(self, Self::Ignored)
    }
}

enum PendingStatus {
    Waiting,
    Ready,
    /// The switch settled without making the target the attached anchor,
    /// or the recorded identities vanished — cancel, never chase.
    Orphaned,
    /// The bounded wait expired — cancel and report.
    Expired,
}

/// Pointer ownership state owned by the dispatcher: the capture flag, the
/// last published hit map, the gesture lifecycle, and the wheel stream.
/// `Default` is released — the shell settles the armed state only after
/// the terminal mode write succeeds.
#[derive(Default)]
pub struct PointerState {
    capture: bool,
    host_unfocused: bool,
    motion_observability: MotionObservability,
    map: Option<FrameHitMap>,
    interactions: interaction::State,
    modal_projection: modal::ProjectionState,
    chrome_projection: chrome::ProjectionState,
    gesture: Gesture,
    cancelled_chrome_release: Option<CancelledChromeRelease>,
    click: Option<SelectionClick>,
    tab_click: Option<tabs::TabClick>,
    close_press: Option<modal::PaintedPress>,
    modal_press: Option<modal::PaintedPress>,
    list_scroll: Option<list_scroll::Pending>,
    copy_drag: Option<copy_mode::Drag>,
    popup_gesture: Option<(
        orchbee_workbench_protocol::WorkbenchRequestId,
        Rect,
        MouseButton,
    )>,
    wheel: WheelStream,
    tooltip: tooltip::State,
}

impl PointerState {
    /// Immutable transient facts used throughout one candidate render.
    pub(crate) fn projection(&self) -> PointerProjection {
        let projection = self
            .interactions
            .projection()
            .with_modal(self.modal_projection());
        self.sidebar_reorder_projection(projection.with_chrome(self.chrome_projection()))
            .with_tooltip(self.tooltip.projection())
    }

    /// Applies the settled outer-transport motion level. Downgrade clears
    /// position and click authority before later input can be admitted.
    pub(crate) fn set_motion_observability(&mut self, observability: MotionObservability) {
        if self.motion_observability == observability {
            return;
        }
        self.motion_observability = observability;
        if observability == MotionObservability::ButtonOnly {
            self.tooltip.clear_transport();
            self.interactions.clear_transport();
            self.cancel_modal_press();
            self.clear_modal_transport();
            self.clear_chrome_transport();
        }
    }

    /// Focus restoration never reconstructs hover from an old position.
    pub(crate) fn set_host_focus(&mut self, focused: bool) {
        self.host_unfocused = !focused;
        if !focused {
            self.tooltip.clear_transport();
            self.interactions.clear_transport();
            self.cancel_modal_press();
            self.clear_modal_transport();
            self.clear_chrome_transport();
            {
                self.cancel_divider();
                self.cancel_sidebar_width();
                self.cancel_tab_drag();
                self.tab_click = None;
                self.cancel_sidebar_reorder();
            }
        }
    }

    pub(crate) fn take_projection_redraw(&mut self) -> bool {
        let interaction = self.interactions.take_redraw_after_publish();
        let modal = self.take_modal_projection_redraw();
        let chrome = self.chrome_projection.take_redraw_after_publish();
        let tooltip = self.tooltip.take_redraw_after_publish();

        interaction || modal || chrome || tooltip
    }

    /// Any key closes the tooltip and still reaches its owner: this never consumes the key.
    pub(crate) fn dismiss_tooltip(&mut self) {
        self.tooltip.dismiss();
    }

    /// Feeds one host report to the tooltip. Only button-free movement under any-motion reporting
    /// can start the rest; a press, wheel notch or drag closes it.
    fn observe_tooltip(&mut self, event: MouseEvent, now: Instant) {
        let cell = (event.column, event.row);
        match (event.action, event.button) {
            (MouseAction::Motion, MouseButton::None) => {
                if self.motion_observability == MotionObservability::AnyMotion && !event.mods.shift
                {
                    self.tooltip.motion(self.map.as_ref(), cell, now);
                }
            }
            (MouseAction::Motion, _) => self.tooltip.dismiss(),
            (MouseAction::Press, _) => self.tooltip.press(self.map.as_ref(), cell),
            (MouseAction::Release, _) => {}
        }
    }

    /// A tooltip may open only on persistent chrome nobody else owns: the pointer is captured and
    /// moving, and no overlay, prefix, mode, gesture or input owner is active.
    fn tooltip_admitted(
        &self,
        view: &ClientView,
        probe: &PointerProbe,
        capture: Option<crate::tui_composition::CaptureOwner>,
    ) -> bool {
        self.capture
            && !self.host_unfocused
            && self.motion_observability == MotionObservability::AnyMotion
            && capture.is_none()
            && !probe.input_captured
            && view
                .overlay()
                .is_none_or(|overlay| overlay == crate::client_view::Overlay::BusinessSidebar)
            && !view.prefix_active()
            && !view.navigator_active()
            && !view.machines.sidebar_navigation_active()
            && !view.resize_mode()
            && matches!(self.gesture, Gesture::Idle)
    }

    pub(crate) fn help_event(
        &mut self,
        view: &mut ClientView,
        event: MouseEvent,
    ) -> PointerOutcome {
        if view.overlay() != Some(crate::client_view::Overlay::ShortcutHelp) {
            return PointerOutcome::Ignored;
        }
        if plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        let interaction = self.modal_interaction_event(event, InteractionTarget::is_help);
        if interaction != PointerOutcome::Ignored {
            return interaction;
        }
        if self.queue_list_scroll(event) {
            return PointerOutcome::Handled;
        }
        if !self.capture || self.host_unfocused || !plain_primary_press(event) {
            return PointerOutcome::Handled;
        }
        let Some(map) = &self.map else {
            return PointerOutcome::Handled;
        };
        if !matches!(
            map.hit(event.column, event.row),
            HitResolution::OverlayBackdrop
        ) {
            return PointerOutcome::Handled;
        }
        let Some(source) = map.help_draw_source().cloned() else {
            return PointerOutcome::Handled;
        };
        let close = crate::client_view::help::DrawnHelpControl::new(source, HelpControl::Close);
        if view.activate_drawn_help(&close) {
            self.stale_map();
            PointerOutcome::RegionsChanged
        } else {
            PointerOutcome::Handled
        }
    }

    pub(crate) fn settings_event(&mut self, event: MouseEvent) -> PointerOutcome {
        if plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        let interaction = self.modal_interaction_event(event, InteractionTarget::is_settings);
        if interaction != PointerOutcome::Ignored {
            return interaction;
        }
        if self.queue_list_scroll(event) {
            return PointerOutcome::Handled;
        }
        PointerOutcome::Ignored
    }

    pub(crate) fn navigator_event(&mut self, event: MouseEvent) -> PointerOutcome {
        if plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        let interaction = self.modal_interaction_event(event, InteractionTarget::is_navigator);
        if interaction != PointerOutcome::Ignored {
            return interaction;
        }
        if self.queue_list_scroll(event) {
            return PointerOutcome::Handled;
        }
        PointerOutcome::Ignored
    }

    pub(crate) fn machine_picker_event(&mut self, event: MouseEvent) -> PointerOutcome {
        if plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        let interaction = self.modal_interaction_event(event, InteractionTarget::is_machine_picker);
        if interaction != PointerOutcome::Ignored {
            return interaction;
        }
        if self.queue_list_scroll(event) {
            return PointerOutcome::Handled;
        }
        PointerOutcome::Ignored
    }

    fn can_admit_interaction(&self) -> bool {
        if !matches!(self.gesture, Gesture::Idle) {
            return false;
        }
        if self.close_press.is_some() || self.modal_press.is_some() {
            return false;
        }
        if self.popup_gesture.is_some() {
            return false;
        }
        true
    }

    fn modal_interaction_event(
        &mut self,
        event: MouseEvent,
        accepts: fn(&InteractionTarget) -> bool,
    ) -> PointerOutcome {
        if !self.capture || self.host_unfocused {
            return match self.interactions.cleanup_event(event) {
                interaction::Event::Ended => PointerOutcome::InteractionEnded,
                interaction::Event::Ignored
                | interaction::Event::Captured
                | interaction::Event::Intent { .. } => PointerOutcome::Ignored,
            };
        }
        let idle_motion = event.action == MouseAction::Motion && event.button == MouseButton::None;
        let interaction_revision = self.interactions.revision();
        let hover_changed = !self.host_unfocused
            && self.interactions.observe_motion(
                self.map.as_ref(),
                self.motion_observability,
                event,
                accepts,
            );
        let admit_new = self.can_admit_interaction();
        match self
            .interactions
            .event(self.map.as_ref(), event, admit_new, accepts)
        {
            interaction::Event::Captured if idle_motion => PointerOutcome::IdleMotion {
                changed: interaction_revision != self.interactions.revision(),
            },
            interaction::Event::Captured => PointerOutcome::Handled,
            interaction::Event::Ended => PointerOutcome::InteractionEnded,
            interaction::Event::Intent { target, contract } => PointerOutcome::Interaction {
                target: *target,
                contract,
            },
            interaction::Event::Ignored if idle_motion => PointerOutcome::IdleMotion {
                changed: hover_changed,
            },
            interaction::Event::Ignored => PointerOutcome::Ignored,
        }
    }
    /// A menu cannot take over a press, pending focus or queued wheel burst.
    pub(crate) fn can_open_action_menu(&self) -> bool {
        matches!(self.gesture, Gesture::Idle | Gesture::TabCancelled)
            && self.wheel.pending == 0
            && self.popup_gesture.is_none()
    }

    /// The drawn cell a keyboard-opened row menu opens beside: the first cell of its row.
    pub(crate) fn sidebar_menu_anchor(
        &self,
        target: &crate::machine_picker::SidebarTarget,
    ) -> Option<(u16, u16)> {
        self.map
            .as_ref()?
            .regions
            .iter()
            .filter(|region| region.target.sidebar_target().as_ref() == Some(target))
            .max_by_key(|region| u32::from(region.rect.width) * u32::from(region.rect.height))
            .map(|region| (region.rect.x, region.rect.y))
    }

    /// Keyboard and pointer entries share one successfully painted row witness.
    pub(crate) fn sidebar_menu_target(
        &self,
        view: &ClientView,
        target: &crate::machine_picker::SidebarTarget,
    ) -> Option<crate::action_menu::MenuTarget> {
        use crate::{action_menu::MenuTarget, machine_picker::SidebarTarget};
        if !self.can_open_action_menu()
            || !view.machines.sidebar_target_exists(target)
            || !view
                .machines
                .sidebar_source()
                .has_part(target, crate::machine_picker::SidebarPart::Menu)
        {
            return None;
        }
        let map = self.map.as_ref()?;
        let source = map.menu_source.as_ref()?;
        let exact = match target {
            SidebarTarget::Project {
                machine,
                session_id,
                project_id,
            } if machine == &view.machines.current && session_id == &source.session => {
                MenuTarget::Project(project_id.clone())
            }
            SidebarTarget::Context { machine, target }
                if machine == &view.machines.current && target.session_id == source.session =>
            {
                MenuTarget::Workspace(target.location.clone())
            }
            _ => return None,
        };
        let drawn = map.regions.iter().any(|region| {
            region.target.sidebar_target().as_ref() == Some(target)
                && matches!(map.hit(region.rect.x, region.rect.y),
                    HitResolution::Region(hit) if hit.sidebar_target().as_ref() == Some(target))
        });
        drawn.then(|| MenuTarget::Drawn {
            source: source.clone(),
            target: Box::new(exact),
        })
    }
    pub(crate) fn action_menu_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        event: MouseEvent,
    ) -> PointerOutcome {
        if plain_primary_press(event) {
            self.cancel_list_scroll();
        }
        let interaction = self.modal_interaction_event(event, InteractionTarget::is_action_menu);
        if let PointerOutcome::IdleMotion { changed } = interaction {
            let candidate_changed = if event.mods == Modifiers::NONE
                && self.motion_observability == MotionObservability::AnyMotion
                && self.modal_release_ownership() == ReleaseOwnership::None
                && self.can_admit_interaction()
                && let Some(InteractionTarget::ActionMenuRow { source, token, row }) =
                    self.interactions.hovered_target()
            {
                port.surface_input(SurfaceInput::ActionMenuHover {
                    source,
                    token: *token,
                    row: *row,
                })
            } else {
                false
            };
            return PointerOutcome::IdleMotion {
                changed: changed || candidate_changed,
            };
        }
        if interaction != PointerOutcome::Ignored {
            return interaction;
        }
        if self.queue_list_scroll(event) {
            return PointerOutcome::Handled;
        }
        if event.mods != crate::client_view::Modifiers::NONE {
            return PointerOutcome::Ignored;
        }
        if event.action != MouseAction::Press {
            return PointerOutcome::Ignored;
        }
        match event.button {
            MouseButton::WheelUp | MouseButton::WheelDown => {
                self.cancel_modal_press();
            }
            MouseButton::Left => {
                if let Some(map) = &self.map
                    && matches!(
                        map.hit(event.column, event.row),
                        HitResolution::OverlayBackdrop
                    )
                {
                    port.surface_action(SurfaceAction::ActionMenu(MenuAction::Cancel));
                    return PointerOutcome::RegionsChanged;
                }
            }
            MouseButton::Right => {
                port.surface_action(SurfaceAction::ActionMenu(MenuAction::Cancel));
                return PointerOutcome::RegionsChanged;
            }
            _ => {}
        }
        PointerOutcome::Ignored
    }

    fn chrome_pressed_fingerprint(&self) -> Option<crate::tui_hit_map::chrome::Fingerprint> {
        match &self.gesture {
            Gesture::Divider(drag) => Some(crate::tui_hit_map::chrome::Fingerprint::divider(
                &drag.projected_target(),
            )),
            Gesture::Tab(drag) => drag.projection_fingerprint(),
            _ => None,
        }
    }

    fn suppress_chrome_hover(&self, pressed: bool) -> bool {
        pressed
            || !matches!(self.gesture, Gesture::Idle)
            || self.cancelled_chrome_release.is_some()
            || self.popup_gesture.is_some()
            || self.modal_release_ownership() != ReleaseOwnership::None
    }

    fn chrome_projection(&self) -> crate::tui_hit_map::chrome::Projection {
        let pressed = self.chrome_pressed_fingerprint();
        let suppress_hover = self.suppress_chrome_hover(pressed.is_some());
        self.chrome_projection.snapshot(pressed, suppress_hover)
    }

    fn observe_chrome_motion(&mut self, event: MouseEvent) -> bool {
        if !self.capture || self.host_unfocused {
            return false;
        }
        let suppress = self.suppress_chrome_hover(false);
        self.chrome_projection.observe_motion(
            self.map.as_ref(),
            self.motion_observability,
            event,
            suppress,
        )
    }

    fn publish_chrome_projection(&mut self, map: &FrameHitMap) -> bool {
        let pressed = self.chrome_pressed_fingerprint();
        let suppress_hover = self.suppress_chrome_hover(pressed.is_some());
        self.chrome_projection.publish(map, pressed, suppress_hover)
    }

    fn invalidate_chrome_projection(&mut self) {
        self.chrome_projection.invalidate_map();
    }

    fn clear_chrome_transport(&mut self) {
        self.chrome_projection.clear_transport();
    }

    /// Read-only geometry shared by drawing and physical-controller viewport observation.
    pub(crate) fn resize_projection(&self) -> crate::tui_layout::ResizeProjection {
        match &self.gesture {
            Gesture::Divider(drag) => {
                return crate::tui_layout::ResizeProjection::Divider {
                    source: drag.target.clone(),
                    ratio: drag.ratio,
                };
            }
            Gesture::SidebarWidth(drag) => {
                return crate::tui_layout::ResizeProjection::Sidebar {
                    source: drag.target,
                    width: drag.width,
                };
            }
            _ => {}
        }
        crate::tui_layout::ResizeProjection::Canonical
    }

    pub(crate) fn resize_presentation_allowed(&self) -> bool {
        self.capture && !self.host_unfocused
    }

    /// Current reversible geometry; shared ratios remain authoritative until release commits.
    pub(crate) fn divider_preview(&self) -> Option<&crate::tui_layout::DividerDrag> {
        match &self.gesture {
            Gesture::Divider(drag) => Some(drag),
            _ => None,
        }
    }

    /// Cancels unsubmitted resize only; a submitted adjustment belongs to ClientControl.
    pub(crate) fn cancel_divider(&mut self) -> bool {
        if matches!(self.gesture, Gesture::Divider(_)) {
            self.gesture = Gesture::Idle;
            self.cancelled_chrome_release
                .get_or_insert(CancelledChromeRelease::Divider);
            true
        } else {
            false
        }
    }

    /// Retires only lower-shell Chrome gestures covered by a newly acquired
    /// compositor owner. Their exact late primary release remains cleanup-only;
    /// pane selection, generic interactions and modal controls keep ownership.
    pub(crate) fn retire_covered_chrome_gesture(&mut self) -> bool {
        if matches!(&self.gesture, Gesture::Divider(_)) {
            return self.cancel_divider();
        }
        if matches!(&self.gesture, Gesture::SidebarWidth(_)) {
            return self.cancel_sidebar_width();
        }
        if self.sidebar_reorder_captured() {
            return self.cancel_sidebar_reorder();
        }
        if matches!(&self.gesture, Gesture::Tab(_)) {
            return self.cancel_tab_drag();
        }
        false
    }

    /// Resolves only the primary-release cleanup retained by an accepted
    /// Windows chrome press. It runs before capture/focus guards so a report
    /// already queued by the host cannot leak after transport teardown.
    fn cancelled_chrome_event(&mut self, event: MouseEvent) -> Option<PointerOutcome> {
        let pending = self.cancelled_chrome_release.is_some()
            || matches!(self.gesture, Gesture::TabCancelled);
        let fresh_press = event.action == MouseAction::Press
            && !matches!(
                event.button,
                MouseButton::WheelUp
                    | MouseButton::WheelDown
                    | MouseButton::WheelLeft
                    | MouseButton::WheelRight
            );
        if fresh_press {
            self.cancelled_chrome_release = None;
            if matches!(self.gesture, Gesture::TabCancelled) {
                self.gesture = Gesture::Idle;
            }
            return None;
        }
        if event.action != MouseAction::Release
            || !matches!(event.button, MouseButton::Left | MouseButton::None)
        {
            return pending.then_some(PointerOutcome::Handled);
        }
        if self.cancelled_chrome_release.take().is_some() {
            return Some(PointerOutcome::ChromeGestureEnded);
        }
        if matches!(self.gesture, Gesture::TabCancelled) {
            self.gesture = Gesture::Idle;
            return Some(PointerOutcome::TabGestureEnded);
        }
        None
    }

    /// Settles a cancelled Client UI press without consulting current hit
    /// geometry or replaying its effect. Callers that receive an ended result
    /// own the rest of the physical input packet.
    pub(crate) fn cancelled_ui_event(&mut self, event: MouseEvent) -> PointerOutcome {
        match self.interactions.cleanup_event(event) {
            interaction::Event::Ended => return PointerOutcome::InteractionEnded,
            interaction::Event::Captured => return PointerOutcome::Handled,
            interaction::Event::Ignored | interaction::Event::Intent { .. } => {}
        }
        if let Some(outcome) = self.cancelled_chrome_event(event) {
            return outcome;
        }
        PointerOutcome::Ignored
    }

    pub(crate) fn popup_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) -> PointerOutcome {
        // An accepted program-body gesture keeps its exact RequestId/content
        // owner. Otherwise the typed Client close interaction gets first
        // refusal before a fresh body report can start a gesture.
        if self.popup_gesture.is_none() {
            let interaction = self.modal_interaction_event(event, InteractionTarget::is_popup);
            if interaction != PointerOutcome::Ignored {
                return interaction;
            }
        }
        if !self.capture || self.host_unfocused || event.mods.shift {
            return PointerOutcome::Ignored;
        }
        let Some(map) = &self.map else {
            return PointerOutcome::Handled;
        };
        if let Some((id, content, button)) = &self.popup_gesture {
            let alive = map.regions.iter().any(|region| {
                matches!(&region.target, HitTarget::Popup { id: painted, content: rect }
                    if painted == id && rect == content)
            });
            if alive
                && matches!(event.action, MouseAction::Motion | MouseAction::Release)
                && (event.button == *button || event.button == MouseButton::None)
            {
                port.surface_input(SurfaceInput::PopupPointer {
                    id,
                    content: *content,
                    event,
                });
                if event.action == MouseAction::Release {
                    self.popup_gesture = None;
                }
                return PointerOutcome::Handled;
            }
            if !alive {
                self.popup_gesture = None;
            }
        }
        match map.hit(event.column, event.row) {
            HitResolution::Region(HitTarget::Popup { id, content }) => {
                if event.action == MouseAction::Release
                    || event.action == MouseAction::Motion && event.button != MouseButton::None
                {
                    return PointerOutcome::Handled;
                }
                port.surface_input(SurfaceInput::PopupPointer {
                    id,
                    content: *content,
                    event,
                });
                if event.action == MouseAction::Press
                    && matches!(
                        event.button,
                        MouseButton::Left
                            | MouseButton::Middle
                            | MouseButton::Right
                            | MouseButton::Extended(_)
                    )
                {
                    self.popup_gesture = Some((id.clone(), *content, event.button));
                }
            }
            HitResolution::Region(HitTarget::PopupClose(_)) => {}
            _ => {}
        }
        PointerOutcome::Handled
    }

    pub(crate) const fn capture_enabled(&self) -> bool {
        self.capture
    }

    /// An external modal surface consumed this decoded mouse event before
    /// ordinary pointer dispatch. A cancelled link press still owns its
    /// matching release, but the modal remains the only effect owner.
    pub(crate) fn external_capture_event(&mut self, event: MouseEvent) {
        let Gesture::Link(link) = &mut self.gesture else {
            return;
        };
        link.cancelled = true;
        if event.action == MouseAction::Release
            && matches!(event.button, MouseButton::Left | MouseButton::None)
        {
            self.gesture = Gesture::Idle;
        }
    }

    fn next_click_count(&self, expected: &PaneRef, cell: PointerSelectionCell, at: Instant) -> u8 {
        if self.click.as_ref().is_some_and(|previous| {
            previous.count == 1
                && previous.expected == *expected
                && previous.cell == cell
                && at
                    .checked_duration_since(previous.at)
                    .is_some_and(|elapsed| elapsed <= DOUBLE_CLICK_INTERVAL)
        }) {
            2
        } else {
            1
        }
    }

    fn begin_selection<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        expected: PaneRef,
        viewport: Rect,
        at: Instant,
        event: MouseEvent,
    ) -> bool {
        let Some(cell) = selection_cell(viewport, event) else {
            return false;
        };
        let click_count = self.next_click_count(&expected, cell, at);
        if port.pane_input(PaneInput::Selection {
            pane: &expected,
            intent: PointerSelectionIntent::Start { click_count, cell },
        }) != PaneAdmission::Admitted
        {
            return false;
        }
        self.click = Some(SelectionClick {
            expected: expected.clone(),
            cell,
            at,
            count: click_count,
        });
        self.gesture = Gesture::Selection(SelectionGesture { expected, viewport });
        true
    }

    /// Cancels only a selection that the port previously admitted. Taking
    /// the state before the effect makes every repeated lifecycle edge a
    /// no-op and prevents reentrant cleanup from emitting a second Cancel.
    pub(crate) fn cancel_selection(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
    ) -> bool {
        let previous = std::mem::replace(&mut self.gesture, Gesture::Idle);
        let Gesture::Selection(selection) = previous else {
            self.gesture = previous;
            return false;
        };
        let _admission = port.pane_input(PaneInput::Selection {
            pane: &selection.expected,
            intent: PointerSelectionIntent::Cancel,
        });
        true
    }

    fn start_focused_press<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        at: Instant,
        event: MouseEvent,
        target: PaneTarget,
    ) -> PointerOutcome {
        let Some(frame) = probe.frame else {
            return PointerOutcome::Ignored;
        };
        let PaneTarget {
            expected,
            content: viewport,
        } = target;
        match route_pane_pointer(
            frame.tracking,
            &event,
            frame.alternate_screen && frame.alternate_scroll,
        ) {
            PanePointerRoute::ProgramReport => {
                if probe.viewport_non_live {
                    return PointerOutcome::Handled;
                }
                if !pane_input_ready(probe, &expected) {
                    view.report_pointer_input_dropped();
                    return PointerOutcome::Handled;
                }
                self.gesture = Gesture::Pane(PaneGesture {
                    button: event.button,
                    expected,
                    viewport,
                    pressed: false,
                    last_cell: None,
                });
                let Gesture::Pane(gesture) = &mut self.gesture else {
                    return PointerOutcome::Handled;
                };
                gesture.forward(view, port, &frame, event);
                PointerOutcome::Handled
            }
            PanePointerRoute::Client if plain_primary_press(event) => {
                if probe.selection_ready {
                    let _admitted = self.begin_selection(port, expected, viewport, at, event);
                }
                PointerOutcome::Handled
            }
            PanePointerRoute::AlternateScroll
            | PanePointerRoute::Client
            | PanePointerRoute::Host => PointerOutcome::Ignored,
        }
    }

    fn selection_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        probe: &PointerProbe,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Some((expected, viewport)) = (match &self.gesture {
            Gesture::Selection(selection) => Some((selection.expected.clone(), selection.viewport)),
            _ => None,
        }) else {
            return PointerOutcome::Ignored;
        };
        if !matches!(&self.gesture, Gesture::Selection(selection) if selection.alive(probe)) {
            self.cancel_selection(port);
            return PointerOutcome::Handled;
        }
        let accepted = match event.action {
            MouseAction::Motion => event.button == MouseButton::Left,
            MouseAction::Release => {
                event.button == MouseButton::Left || event.button == MouseButton::None
            }
            MouseAction::Press => false,
        };
        if !accepted {
            return PointerOutcome::Handled;
        }
        let Some(cell) = selection_cell(viewport, event) else {
            self.cancel_selection(port);
            return PointerOutcome::Handled;
        };
        match event.action {
            MouseAction::Motion => {
                let _admission = port.pane_input(PaneInput::Selection {
                    pane: &expected,
                    intent: PointerSelectionIntent::Update { cell },
                });
            }
            MouseAction::Release => {
                self.gesture = Gesture::Idle;
                let _admission = port.pane_input(PaneInput::Selection {
                    pane: &expected,
                    intent: PointerSelectionIntent::End { cell },
                });
            }
            MouseAction::Press => {}
        }
        PointerOutcome::Handled
    }

    /// The lifecycle write succeeded — the flag mirrors the armed state.
    /// Releasing is immediate: in-flight gestures and the wheel stream
    /// die so nothing reaches a pane after the host stops reporting.
    pub(crate) fn set_capture(&mut self, enabled: bool) {
        if !enabled {
            self.tooltip.clear_transport();
            self.cancel_list_scroll();
            self.cancel_modal_press();
            self.interactions.clear_transport();
            self.clear_modal_transport();
            self.clear_chrome_transport();
        }
        if self.capture == enabled {
            return;
        }
        self.capture = enabled;
        if !enabled {
            {
                self.cancel_divider();
                self.cancel_sidebar_width();
                self.cancel_tab_drag();
                self.tab_click = None;
                self.cancel_sidebar_reorder();
            }
            if let Gesture::Link(link) = &mut self.gesture {
                // Keep a cancelled tombstone until the matching release so
                // re-arming capture cannot reinterpret that release as a
                // fresh pane event.
                link.cancelled = true;
            } else if !matches!(self.gesture, Gesture::TabCancelled) {
                self.gesture = Gesture::Idle;
            }

            {
                self.popup_gesture = None;
            }
            self.wheel.clear();
        }
    }

    /// Port-bearing lifecycle seam used when releasing capture can owe a
    /// cancellation to the exact pane that admitted a selection.
    pub(crate) fn set_capture_with_port(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        enabled: bool,
    ) {
        if !enabled {
            self.cancel_selection(port);
        }
        self.set_capture(enabled);
    }

    /// The draw completed — the recorded map becomes the resolution
    /// authority until the next layout change.
    pub(crate) fn publish(&mut self, map: FrameHitMap) {
        if self
            .copy_drag
            .as_ref()
            .is_some_and(|drag| !drag.painted_in(&map))
        {
            self.cancel_copy_drag();
        }
        if self
            .list_scroll
            .as_ref()
            .is_some_and(|pending| !pending.exposed_in(&map))
        {
            self.cancel_list_scroll();
        }
        if matches!(&self.gesture, Gesture::Tab(drag) if !drag.painted_in(&map)) {
            self.cancel_tab_drag();
            self.cancel_sidebar_reorder();
        }
        if matches!(&self.gesture, Gesture::SidebarReorder(drag) if !drag.painted_in(&map)) {
            self.cancel_sidebar_reorder();
        }
        for press in [&mut self.close_press, &mut self.modal_press]
            .into_iter()
            .flatten()
        {
            if !press.exposed_in(&map) {
                press.cancel();
            }
        }
        if let Gesture::Scrollbar(drag) = &self.gesture
            && !drag.painted_in(&map)
        {
            self.gesture = Gesture::Idle;
        }
        let divider_lost = if let Gesture::Divider(drag) = &mut self.gesture {
            let painted = drag.projected_target();
            if map.exposes_divider(&painted) {
                drag.painted = painted;
                false
            } else {
                true
            }
        } else {
            false
        };
        if divider_lost {
            self.cancel_divider();
        }
        let sidebar_lost = if let Gesture::SidebarWidth(drag) = &mut self.gesture {
            let painted = drag.projected_target();
            if map.exposes_sidebar_width(&painted) {
                drag.painted = painted;
                false
            } else {
                true
            }
        } else {
            false
        };
        if sidebar_lost {
            self.cancel_sidebar_width();
        }
        if let Gesture::Link(link) = &mut self.gesture
            && !link.source_matches(&map)
        {
            // The matching release remains pointer-owned, but a covered or
            // replaced source can no longer hand off an external effect.
            link.cancelled = true;
        }
        self.interactions.publish(&map);
        self.publish_modal_projection(&map);
        self.publish_chrome_projection(&map);
        self.tooltip.publish(&map);
        self.map = Some(map);
    }

    pub(crate) fn navigator_paint(&self) -> Option<&crate::client_view::navigator::Paint> {
        self.map.as_ref()?.navigator_paint()
    }

    pub(crate) fn navigator_backdrop_at(&self, column: u16, row: u16) -> bool {
        self.map.as_ref().is_some_and(|map| {
            map.navigator_paint().is_some()
                && matches!(map.hit(column, row), HitResolution::OverlayBackdrop)
        })
    }

    /// The layout changed outside the pointer path — a resize, a
    /// keyboard-driven transition, a half-painted draw: the recorded
    /// regions no longer describe the screen, an in-flight gesture holds
    /// press-time geometry that no longer matches, and the wheel stream
    /// holds the old viewport bound. All three cancel; between now and
    /// the next publish no new gesture is admitted, while the exact release
    /// of an already accepted Client UI press may still settle cleanup once.
    pub(crate) fn invalidate_map(&mut self) {
        self.tooltip.dismiss();
        self.cancel_list_scroll();
        self.cancel_modal_press();
        self.interactions.invalidate_map();
        self.invalidate_modal_projection();
        self.invalidate_chrome_projection();
        self.map = None;
        {
            self.popup_gesture = None;
        }
        {
            self.cancel_divider();
            self.cancel_sidebar_width();
            self.cancel_tab_drag();
            self.cancel_sidebar_reorder();
        }
        if let Gesture::Link(link) = &mut self.gesture {
            link.cancelled = true;
        } else if !matches!(self.gesture, Gesture::TabCancelled) {
            self.gesture = Gesture::Idle;
        }

        self.wheel.clear();
    }

    /// Cancels an admitted start before a structural invalidation clears
    /// the recorded map and press-time state.
    pub(crate) fn invalidate_map_with_port(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
    ) {
        self.cancel_selection(port);
        self.invalidate_map();
    }

    /// A handled pointer event changed structural regions (navigation
    /// applied or an overlay dismissed): the map stales until the next draw.
    /// Dedicated gesture owners retain cleanup authority; a generic click
    /// arm cancels because its painted interaction authority is gone and
    /// retains only matching-release cleanup. The
    /// dispatcher calls this on `RegionsChanged`; the map itself never
    /// decides.
    pub(crate) fn stale_map(&mut self) {
        self.tooltip.dismiss();
        self.cancel_modal_press();
        self.interactions.invalidate_map();
        self.invalidate_modal_projection();
        self.invalidate_chrome_projection();
        self.map = None;
    }

    /// One decoded host report. `Handled`/`RegionsChanged` both ask the
    /// dispatcher to run the same viewport-sync path a keyboard action
    /// would; `RegionsChanged` additionally marks the recorded map stale
    /// — the gesture the event belongs to is untouched either way.
    pub(crate) fn event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        event: MouseEvent,
    ) -> PointerOutcome {
        if !self.capture || self.host_unfocused {
            return self.cancelled_ui_event(event);
        }
        let cancelled = self.cancelled_ui_event(event);
        if cancelled != PointerOutcome::Ignored {
            return cancelled;
        }
        let idle_motion = event.action == MouseAction::Motion && event.button == MouseButton::None;
        let probe = port.pointer_probe();
        if probe.input_captured {
            // A modal draft or an armed confirmation owns input: pointer
            // events are swallowed without producing gestures.
            self.tooltip.dismiss();
            self.interactions.clear_transport();
            self.clear_chrome_transport();
            self.cancel_selection(port);
            {
                self.cancel_divider();
                self.cancel_sidebar_width();
                self.cancel_tab_drag();
                self.cancel_sidebar_reorder();
            }
            let cancelled = self.cancelled_ui_event(event);
            if cancelled != PointerOutcome::Ignored {
                return cancelled;
            }
            if let Gesture::Link(link) = &mut self.gesture {
                link.cancelled = true;
                if event.action == MouseAction::Release
                    && matches!(event.button, MouseButton::Left | MouseButton::None)
                {
                    self.gesture = Gesture::Idle;
                }
                self.wheel.clear();
                view.machines.cancel_sidebar_scroll();
                return PointerOutcome::Handled;
            }
            self.gesture = Gesture::Idle;
            self.wheel.clear();
            view.machines.cancel_sidebar_scroll();
            return PointerOutcome::Ignored;
        }
        self.observe_tooltip(event, now);
        let interaction_revision = self.interactions.revision();
        let hover_changed = !self.host_unfocused
            && self.interactions.observe_motion(
                self.map.as_ref(),
                self.motion_observability,
                event,
                InteractionTarget::is_global_chrome,
            );
        let hover_changed = hover_changed | self.observe_chrome_motion(event);
        if self.sidebar_reorder_captured() {
            return self.sidebar_reorder_event(view, port, event);
        }
        if self.begin_sidebar_reorder(view, port, now, event) {
            return PointerOutcome::Handled;
        }
        let admit_new = self.can_admit_interaction();
        match self.interactions.event(
            self.map.as_ref(),
            event,
            admit_new,
            InteractionTarget::is_global_chrome,
        ) {
            interaction::Event::Captured if idle_motion => {
                return PointerOutcome::IdleMotion {
                    changed: interaction_revision != self.interactions.revision(),
                };
            }
            interaction::Event::Captured => return PointerOutcome::Handled,
            interaction::Event::Ended => return PointerOutcome::InteractionEnded,
            interaction::Event::Intent { target, contract } => {
                return PointerOutcome::Interaction {
                    target: *target,
                    contract,
                };
            }
            interaction::Event::Ignored => {}
        }
        let outcome = match self.gesture {
            Gesture::Tab(_) | Gesture::TabCancelled => self.tab_event(view, port, now, event),
            Gesture::SidebarReorder(_) => self.sidebar_reorder_event(view, port, event),
            Gesture::Pending(_) => self.pending_event(view, port, &probe, now, event),
            Gesture::Pane(_) => self.pane_event(view, port, &probe, event),
            Gesture::Selection(_) => self.selection_event(port, &probe, event),
            Gesture::Scrollbar(_) => self.scrollbar_event(port, &probe, event),
            Gesture::Divider(_) => self.divider_event(view, port, event),
            Gesture::SidebarWidth(_) => self.sidebar_width_event(view, event),
            Gesture::Link(_) => self.link_event(port, &probe, event),
            Gesture::Idle => self.fresh_event(view, port, &probe, now, event),
        };
        if idle_motion && outcome == PointerOutcome::Ignored {
            PointerOutcome::IdleMotion {
                changed: hover_changed,
            }
        } else {
            outcome
        }
    }

    /// Advances pending-focus readiness and the wheel flush between
    /// packets and on idle passes. Returns `true` when bytes moved or a
    /// gesture settled — the caller redraws.
    pub fn tick<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        capture: Option<crate::tui_composition::CaptureOwner>,
    ) -> bool {
        let probe = port.pointer_probe();
        let admitted = self.tooltip_admitted(view, &probe, capture);
        let tooltip_changed = self.tooltip.tick(self.map.as_ref(), now, admitted);
        if !self.capture
            || probe.input_captured
            || view
                .overlay()
                .is_some_and(|o| o != crate::client_view::Overlay::BusinessSidebar)
        {
            view.machines.cancel_sidebar_scroll();
        }
        if probe.input_captured {
            let gesture_changed = match &self.gesture {
                Gesture::Idle => false,
                Gesture::Link(link) => !link.cancelled,
                _ => true,
            };
            let interaction_revision = self.interactions.revision();
            // Native capture blocks pane gestures, but a menu's own painted click remains
            // owned across idle and draw boundaries. The dispatcher supplies the current
            // composition winner; map publication still checks its exact fingerprint.
            let menu_owns_interaction = matches!(
                capture,
                Some(crate::tui_composition::CaptureOwner::Native(
                    crate::tui_composition::WorkbenchCaptureOwner::ActionMenu
                ))
            );

            if !menu_owns_interaction {
                self.interactions.clear_transport();
            }
            let changed = gesture_changed
                || self.wheel.pending != 0
                || interaction_revision != self.interactions.revision();
            self.cancel_selection(port);
            {
                self.cancel_divider();
                self.cancel_sidebar_width();
                self.cancel_tab_drag();
                self.cancel_sidebar_reorder();
            }
            if let Gesture::Link(link) = &mut self.gesture {
                link.cancelled = true;
            } else if !matches!(self.gesture, Gesture::TabCancelled) {
                self.gesture = Gesture::Idle;
            }

            self.wheel.clear();
            return changed || tooltip_changed;
        }
        let mut changed = tooltip_changed;
        if matches!(&self.gesture, Gesture::Tab(drag) if port.tab_drag_source().as_ref() != Some(&drag.source))
            || (self.tab_drag_captured()
                && (view.overlay().is_some_and(|overlay| {
                    overlay != crate::client_view::Overlay::BusinessSidebar
                }) || view.resize_mode()))
        {
            changed |= self.cancel_tab_drag();
            self.cancel_sidebar_reorder();
        }
        {
            changed |= self.tick_sidebar_reorder(view, port, now);
        }
        let modal = view.overlay().is_some();
        let modal = modal || view.resize_mode();
        if let Gesture::Scrollbar(drag) = &self.gesture
            && (!drag.alive(&probe) || modal)
        {
            self.gesture = Gesture::Idle;
            changed = true;
        }
        if self
            .sidebar_width_preview()
            .is_some_and(|drag| !self.sidebar_width_source_alive(view, &drag.target))
        {
            changed |= self.cancel_sidebar_width();
        }
        if self.divider_preview().is_some_and(|drag| {
            view.overlay().is_some()
                || view.resize_mode()
                || view.machines.open
                || self
                    .map
                    .as_ref()
                    .is_none_or(|map| !map.exposes_divider(&drag.painted))
        }) {
            changed |= self.cancel_divider();
        }
        if let Some(drag) = self.divider_preview()
            && let Err(notice) = port.pointer_divider_ready(&drag.target)
        {
            changed |= self.cancel_divider();
            view.configuration_notice(notice);
        }
        if matches!(&self.gesture, Gesture::Selection(selection) if !selection.alive(&probe)) {
            changed |= self.cancel_selection(port);
        }
        if view.machines.settle_sidebar_scroll() {
            // The window changed at cadence, not per report; old row geometry drops.
            self.stale_map();
            changed = true;
        }
        if matches!(self.gesture, Gesture::Pending(_)) {
            changed |= self.settle_pending(view, port, &probe, now);
        }
        // A drained stream closes at the pass boundary; the next notch
        // then starts a fresh burst rather than reversing the old one.
        if self.wheel.pending == 0 {
            self.wheel.up = None;
        }
        self.wheel.emitted = 0;
        changed |= self.flush_wheel(view, port);
        changed
    }

    fn flush_wheel<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
    ) -> bool {
        let probe = port.pointer_probe();
        if probe.viewport_non_live
            || !probe.frame.is_some_and(|frame| {
                frame.alternate_screen
                    && frame.alternate_scroll
                    && frame.tracking == MouseTracking::None
            })
        {
            self.wheel.clear();
            return false;
        }
        let Some((up, count, application_cursor)) = self.wheel.take_flush() else {
            return false;
        };
        let bytes = encode_cursor_arrows(up, count, application_cursor);
        forward_or_report(view, &bytes, port);
        // Anything left pending flushes on a later pass — the bounded
        // remainder, not a dropped tail.
        true
    }

    fn fresh_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        now: Instant,
        event: MouseEvent,
    ) -> PointerOutcome {
        // Shift gestures remain owned by the host terminal.
        if event.mods.shift {
            return PointerOutcome::Ignored;
        }
        let Some(map) = self.map.as_ref() else {
            return PointerOutcome::Ignored;
        };
        if event.action == MouseAction::Press
            && !matches!(
                map.hit(event.column, event.row),
                HitResolution::Region(HitTarget::Tab(_))
            )
        {
            self.tab_click = None;
        }
        if event.action == MouseAction::Press
            && event.mods == crate::client_view::Modifiers::NONE
            && !view.resize_mode()
            && !view.machines.open
            && self.can_open_action_menu()
        {
            let target = match map.hit(event.column, event.row) {
                HitResolution::Region(HitTarget::CreateMenu(target))
                    if event.button == MouseButton::Left =>
                {
                    match target {
                        None => Some(crate::action_menu::MenuTarget::Create(None)),
                        Some(target) => self.sidebar_menu_target(view, target).and_then(|target| {
                            let crate::action_menu::MenuTarget::Drawn { source, target } = target
                            else {
                                return None;
                            };
                            let crate::action_menu::MenuTarget::Project(project) = *target else {
                                return None;
                            };
                            Some(crate::action_menu::MenuTarget::Drawn {
                                source,
                                target: Box::new(crate::action_menu::MenuTarget::Create(Some(
                                    project,
                                ))),
                            })
                        }),
                    }
                }
                HitResolution::Region(HitTarget::TabGroup { tab, .. })
                    if event.button == MouseButton::Right =>
                {
                    Some(crate::action_menu::MenuTarget::TabGroups {
                        tab_id: tab.clone(),
                        colors: false,
                    })
                }
                HitResolution::Region(HitTarget::Tab(tab))
                    if event.button == MouseButton::Right =>
                {
                    Some(crate::action_menu::MenuTarget::Tab(tab.clone()))
                }
                HitResolution::Region(HitTarget::TabCreate(target))
                    if event.button == MouseButton::Right =>
                {
                    Some(crate::action_menu::MenuTarget::TabProfiles(target.clone()))
                }
                HitResolution::Region(hit)
                    if hit.sidebar_target().is_some() && event.button == MouseButton::Right =>
                {
                    hit.sidebar_target()
                        .and_then(|target| self.sidebar_menu_target(view, &target))
                }
                HitResolution::Region(HitTarget::Pane { pane, content })
                    if event.button == MouseButton::Right =>
                {
                    let policy = probe
                        .panes
                        .iter()
                        .find(|p| p.record == *pane)
                        .map(|p| p.right_click);
                    if policy.is_some()
                        && (!contains(*content, event.column, event.row)
                            || policy
                                != Some(orchbee_workbench_protocol::PanelRightClickTarget::Panel))
                    {
                        Some(crate::action_menu::MenuTarget::Pane(pane.clone()))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(target) = target {
                let target = if matches!(
                    target,
                    crate::action_menu::MenuTarget::Global
                        | crate::action_menu::MenuTarget::Drawn { .. }
                ) {
                    target
                } else {
                    let Some(source) = map.menu_source.clone() else {
                        return PointerOutcome::Handled;
                    };
                    crate::action_menu::MenuTarget::Drawn {
                        source,
                        target: Box::new(target),
                    }
                };
                return PointerOutcome::MenuRequested {
                    target,
                    anchor: (event.column, event.row),
                    button: event.button,
                };
            }
        }
        if event.action == MouseAction::Press
            && event.button == MouseButton::Left
            && event.mods == crate::client_view::Modifiers::NONE
            && view.overlay() != Some(crate::client_view::Overlay::ShortcutHelp)
            && !matches!(map.hit(event.column, event.row),
                HitResolution::Region(hit) if hit.sidebar_target().is_some()
                    || matches!(hit, HitTarget::MachineWorkspaceSurface(_) | HitTarget::SidebarResize(_) | HitTarget::TabCreate(_)))
        {
            view.machines.leave_sidebar_navigation();
        }
        match map.hit(event.column, event.row) {
            HitResolution::Region(HitTarget::SettingsButton | HitTarget::NewWorkspaceButton) => {
                PointerOutcome::Ignored
            }
            HitResolution::Region(HitTarget::Scrollbar {
                pane,
                content,
                source,
            }) => {
                let (pane, content, source) = (pane.clone(), *content, *source);
                self.begin_scrollbar(view, port, probe, event, pane, content, source)
            }
            HitResolution::Region(HitTarget::SidebarResize(target)) => {
                let target = *target;
                self.begin_sidebar_width(view, target, event)
            }
            HitResolution::Region(
                HitTarget::CreateMenu(_)
                | HitTarget::Onboarding(_)
                | HitTarget::OnboardingSurface
                | HitTarget::SidebarRestoreButton
                | HitTarget::ActionMenuRow { .. }
                | HitTarget::Worktree { .. }
                | HitTarget::NavigatorRow(_)
                | HitTarget::NavigatorClose(_)
                | HitTarget::MachinePicker(_)
                | HitTarget::TabCreate(_)
                | HitTarget::Resize(_)
                | HitTarget::CopyControl { .. }
                | HitTarget::CopySurface { .. }
                | HitTarget::Presentation(_),
            ) => PointerOutcome::Ignored,
            HitResolution::Region(HitTarget::SettingsRow { .. }) => PointerOutcome::Ignored,
            HitResolution::Region(HitTarget::Divider(target)) => {
                if event.action != MouseAction::Press
                    || event.button != MouseButton::Left
                    || event.mods != crate::client_view::Modifiers::NONE
                {
                    return PointerOutcome::Ignored;
                }
                if let Err(notice) = port.pointer_divider_ready(target) {
                    view.configuration_notice(notice);
                    return PointerOutcome::Handled;
                }
                if let Some(drag) =
                    crate::tui_layout::DividerDrag::begin(target.clone(), event.column, event.row)
                {
                    self.click = None;
                    self.wheel.clear();
                    self.gesture = Gesture::Divider(drag);
                }
                PointerOutcome::Handled
            }
            #[cfg(test)]
            HitResolution::Region(HitTarget::MenuButton) => PointerOutcome::Ignored,
            HitResolution::Dead => PointerOutcome::Ignored,
            HitResolution::OverlayBackdrop => {
                // An outside click on a capturing overlay dismisses it and
                // dies here — nothing reaches the regions beneath.
                if event.action == MouseAction::Press
                    && event.mods == Modifiers::NONE
                    && matches!(
                        event.button,
                        MouseButton::Left | MouseButton::Right | MouseButton::Middle
                    )
                {
                    view.dismiss_overlay();
                    // The board's own wheel guard: an outside wheel notch
                    // is swallowed whole — only an ordinary outside Press
                    // dismisses the board. Other overlays keep their
                    // existing backdrop semantics above.
                    if !matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown) {
                        port.surface_action(SurfaceAction::AgentBoard(BoardAction::Dismiss));
                    }
                    return PointerOutcome::RegionsChanged;
                }
                PointerOutcome::Ignored
            }
            HitResolution::Region(HitTarget::Popup { .. } | HitTarget::PopupClose(_)) => {
                PointerOutcome::Ignored
            }
            HitResolution::Region(
                HitTarget::OverlaySurface
                | HitTarget::Setup { .. }
                | HitTarget::WorkspaceClose { .. }
                | HitTarget::TabClose { .. }
                | HitTarget::PanelClose { .. }
                | HitTarget::Rename { .. }
                | HitTarget::SettingsControl { .. }
                | HitTarget::AgentBoardControl(_)
                | HitTarget::AgentBoardRow(_)
                | HitTarget::ListScroll(_)
                | HitTarget::Help { .. },
            ) => PointerOutcome::Ignored,
            HitResolution::Region(
                HitTarget::SidebarMenu(_)
                | HitTarget::SidebarClose(_)
                | HitTarget::ActionMenuControl { .. }
                | HitTarget::TabCloseRequest { .. }
                | HitTarget::TabStripPage { .. },
            ) => PointerOutcome::Ignored,
            HitResolution::Region(HitTarget::MachineGroupRow { window, .. }) => {
                if event.action != MouseAction::Press
                    || event.mods != crate::client_view::Modifiers::NONE
                {
                    return PointerOutcome::Ignored;
                }
                match event.button {
                    MouseButton::WheelUp | MouseButton::WheelDown => {
                        view.machines
                            .queue_space_scroll(*window, event.button == MouseButton::WheelUp);
                        PointerOutcome::Handled
                    }
                    _ => PointerOutcome::Ignored,
                }
            }
            HitResolution::Region(HitTarget::SidebarDisclosure(_)) => PointerOutcome::Ignored,
            HitResolution::Region(HitTarget::OpenMachineWorkspaces) => PointerOutcome::Ignored,
            HitResolution::Region(HitTarget::MachineWorkspaceRow { window, .. }) => {
                if event.action != MouseAction::Press
                    || event.mods != crate::client_view::Modifiers::NONE
                {
                    return PointerOutcome::Ignored;
                }
                match event.button {
                    MouseButton::WheelUp | MouseButton::WheelDown => {
                        view.machines
                            .queue_space_scroll(*window, event.button == MouseButton::WheelUp);
                        PointerOutcome::Handled
                    }
                    _ => PointerOutcome::Ignored,
                }
            }
            HitResolution::Region(HitTarget::MachineWorkspaceSurface(window)) => {
                if event.action == MouseAction::Press
                    && event.mods == crate::client_view::Modifiers::NONE
                    && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
                {
                    view.machines
                        .queue_space_scroll(*window, event.button == MouseButton::WheelUp);
                    PointerOutcome::Handled
                } else {
                    PointerOutcome::Ignored
                }
            }
            HitResolution::Region(HitTarget::SearchButton) => PointerOutcome::Ignored,
            HitResolution::Region(HitTarget::MachineAgentRow { window, .. }) => {
                if event.action != MouseAction::Press
                    || event.mods != crate::client_view::Modifiers::NONE
                {
                    return PointerOutcome::Ignored;
                }
                match event.button {
                    MouseButton::WheelUp | MouseButton::WheelDown => {
                        view.machines
                            .queue_space_scroll(*window, event.button == MouseButton::WheelUp);
                        PointerOutcome::Handled
                    }
                    _ => PointerOutcome::Ignored,
                }
            }
            HitResolution::Region(target @ HitTarget::TabGroup { .. }) => {
                if event.action != MouseAction::Press
                    || event.button != MouseButton::Left
                    || event.mods != crate::client_view::Modifiers::NONE
                {
                    return PointerOutcome::Ignored;
                }
                if let Some(drag) = port
                    .tab_drag_source()
                    .and_then(|source| tabs::TabDrag::begin(source, map, target.clone(), event))
                {
                    self.click = None;
                    self.wheel.clear();
                    self.gesture = Gesture::Tab(drag);
                }
                PointerOutcome::Handled
            }
            HitResolution::Region(HitTarget::Tab(tab)) => {
                if event.action != MouseAction::Press || event.button != MouseButton::Left {
                    return PointerOutcome::Ignored;
                }
                let Some(_index) = probe.tab_index(tab) else {
                    return PointerOutcome::Ignored;
                };
                if event.mods == Modifiers::CTRL {
                    let disposition = port.apply_client_action(
                        (ClientAction::TabMenu {
                            action: crate::action_menu::MenuAction::SelectTab,
                            tab_id: Some(tab.clone()),
                        })
                        .into(),
                    );
                    if let Some(disposition) = disposition {
                        view.commit_pointer(disposition);
                    }
                    return PointerOutcome::RegionsChanged;
                }
                if event.mods != Modifiers::NONE {
                    return PointerOutcome::Ignored;
                }
                {
                    if let Some(drag) = port.tab_drag_source().and_then(|source| {
                        tabs::TabDrag::begin(source, map, HitTarget::Tab(tab.clone()), event)
                    }) {
                        self.click = None;
                        self.wheel.clear();
                        self.gesture = Gesture::Tab(drag);
                    }
                    PointerOutcome::Handled
                }
            }
            HitResolution::Region(HitTarget::Unbound {
                selected,
                presentation,
            }) => {
                if event.action != MouseAction::Press
                    || event.button != MouseButton::Left
                    || event.mods != crate::client_view::Modifiers::NONE
                    || probe
                        .desired
                        .as_ref()
                        .is_none_or(|desired| desired.anchor() != selected.key.anchor())
                    || !probe.unbound.contains(selected)
                    || presentation.is_some()
                {
                    return PointerOutcome::Ignored;
                }
                if let Some(disposition) = port.apply_client_action(
                    (ClientAction::FocusPanel(selected.key.panel().clone())).into(),
                ) {
                    view.commit_pointer(disposition);
                }
                PointerOutcome::RegionsChanged
            }
            HitResolution::Region(HitTarget::Pane { pane, content }) => {
                if link_press(event) && contains(*content, event.column, event.row) {
                    let cell = PointerSelectionCell {
                        column: event.column - content.x,
                        row: event.row - content.y,
                    };
                    let source = map
                        .link_source(pane, event.column, event.row)
                        .cloned()
                        .filter(|source| {
                            probe
                                .panes
                                .iter()
                                .any(|live| live.record == source.pane && live.attached)
                        })
                        .filter(|source| source.rows.point(cell.column, cell.row).is_some());
                    let cancelled = source.is_none();
                    self.click = None;
                    self.wheel.clear();
                    self.gesture = Gesture::Link(LinkGesture {
                        source,
                        viewport: *content,
                        press_column: event.column,
                        press_row: event.row,
                        cell,
                        cancelled,
                    });
                    return PointerOutcome::Handled;
                }
                let target = PaneTarget {
                    expected: pane.clone(),
                    content: *content,
                };
                self.pane_press(view, port, probe, now, event, target)
            }
        }
    }

    fn divider_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::Divider(drag) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        if event.mods.shift {
            self.cancel_divider();
            return self
                .cancelled_chrome_event(event)
                .unwrap_or(PointerOutcome::Ignored);
        }
        if self
            .map
            .as_ref()
            .is_none_or(|map| !map.exposes_divider(&drag.painted))
        {
            self.cancel_divider();
            return self
                .cancelled_chrome_event(event)
                .unwrap_or(PointerOutcome::Handled);
        }
        if let Err(notice) = port.pointer_divider_ready(&drag.target) {
            self.cancel_divider();
            view.configuration_notice(notice);
            return self
                .cancelled_chrome_event(event)
                .unwrap_or(PointerOutcome::Handled);
        }
        match event.action {
            MouseAction::Motion if event.button == MouseButton::Left => {
                drag.update(event.column, event.row);
            }
            MouseAction::Release
                if matches!(event.button, MouseButton::Left | MouseButton::None) =>
            {
                let Gesture::Divider(mut drag) = std::mem::take(&mut self.gesture) else {
                    unreachable!()
                };
                drag.update(event.column, event.row);
                if drag.ratio != drag.target.ratio {
                    port.surface_action(SurfaceAction::Layout(LayoutAction::Divider {
                        target: &drag.target,
                        ratio: drag.ratio,
                    }));
                }
                return PointerOutcome::ChromeGestureEnded;
            }
            _ => {}
        }
        PointerOutcome::Handled
    }

    fn link_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        probe: &PointerProbe,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::Link(link) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        let same_cell = event.column == link.press_column && event.row == link.press_row;
        let current_source_matches = self
            .map
            .as_ref()
            .is_some_and(|map| link.source_matches(map));
        if !current_source_matches
            || !link.alive(probe)
            || event.mods.shift
            || event.mods.alt
            || event.mods.super_
            || event.action == MouseAction::Motion
        {
            link.cancelled = true;
        }
        match event.action {
            MouseAction::Motion => PointerOutcome::Handled,
            MouseAction::Press => {
                link.cancelled = true;
                PointerOutcome::Handled
            }
            MouseAction::Release
                if matches!(event.button, MouseButton::Left | MouseButton::None) =>
            {
                let previous = std::mem::replace(&mut self.gesture, Gesture::Idle);
                let Gesture::Link(link) = previous else {
                    return PointerOutcome::Handled;
                };
                if same_cell
                    && !link.cancelled
                    && let Some(source) = link.source
                {
                    let _admission = port.pane_input(PaneInput::Link {
                        source: Box::new(source),
                        cell: link.cell,
                    });
                }
                PointerOutcome::Handled
            }
            MouseAction::Release => {
                link.cancelled = true;
                PointerOutcome::Handled
            }
        }
    }

    /// A press landing on a pane region: focus first (Client-local, before
    /// any forwarding), then the pane decides ownership from its reported
    /// contract. A press on the already-focused incarnation starts a
    /// `Pane` gesture when the pane reports the pointer; a press on a
    /// sibling pane drives the local focus transition and settles against
    /// the *recorded* incarnation — never a positional index or a fresh
    /// record minted after the paint.
    fn pane_press<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        now: Instant,
        event: MouseEvent,
        target: PaneTarget,
    ) -> PointerOutcome {
        let PaneTarget { expected, content } = target;
        if event.button == MouseButton::Right
            && event.action == MouseAction::Press
            && (event.mods != crate::client_view::Modifiers::NONE
                || !contains(content, event.column, event.row)
                || !probe.panes.iter().any(|pane| {
                    pane.record == expected
                        && pane.right_click
                            == orchbee_workbench_protocol::PanelRightClickTarget::Panel
                        && pane.right_click_reporting
                }))
        {
            return PointerOutcome::Ignored;
        }
        if matches!(event.action, MouseAction::Press)
            && matches!(
                event.button,
                MouseButton::WheelUp
                    | MouseButton::WheelDown
                    | MouseButton::WheelLeft
                    | MouseButton::WheelRight
            )
        {
            return self.wheel_on_pane(view, port, probe, event, &PaneTarget { expected, content });
        }
        if event.action != MouseAction::Press {
            // A non-Live focus owns every non-wheel pointer event locally:
            // motion and release inside its content die here just like the
            // press did: they mint no gesture and encode no bytes.
            if probe.focus.as_ref() == Some(&expected)
                && probe.viewport_non_live
                && probe.frame.is_some()
                && contains(content, event.column, event.row)
            {
                return PointerOutcome::Handled;
            }
            // A fresh motion/release cannot inherit a pane gesture: held
            // motion and release never retarget when their press began
            // elsewhere. Any tracking is the sole stateless exception,
            // reporting button-free motion to the exact already-focused,
            // live, controlling pane recorded by the painted region.
            if event.action != MouseAction::Motion
                || event.button != MouseButton::None
                || !contains(content, event.column, event.row)
                || !pane_input_ready(probe, &expected)
            {
                return PointerOutcome::Ignored;
            }
            let Some(frame) = probe.frame else {
                return PointerOutcome::Ignored;
            };
            if route_pane_pointer(
                frame.tracking,
                &event,
                frame.alternate_screen && frame.alternate_scroll,
            ) != PanePointerRoute::ProgramReport
            {
                return PointerOutcome::Ignored;
            }
            let mut reporter = PaneGesture {
                button: MouseButton::None,
                expected,
                viewport: content,
                pressed: false,
                last_cell: None,
            };
            reporter.forward(view, port, &frame, event);
            return PointerOutcome::Handled;
        }
        let Some(tab_index) = probe.tab_index(expected.anchor().tab_id()) else {
            return PointerOutcome::Ignored;
        };
        if probe
            .desired
            .as_ref()
            .map(|desired| desired.anchor().tab_id())
            != Some(expected.anchor().tab_id())
        {
            // Only the active tab paints panes; a stale-map edge resolves
            // to a plain tab switch instead of an unreachable panel focus.
            let Ok(position) = u8::try_from(probe.tabs[tab_index].position) else {
                return PointerOutcome::Ignored;
            };
            let disposition = port.apply_client_action((ClientAction::SelectTab(position)).into());
            if let Some(disposition) = disposition {
                view.commit_pointer(disposition);
            }
            return PointerOutcome::RegionsChanged;
        }
        let Some(leaf) = probe
            .panes
            .iter()
            .find(|pane| pane.record.pane() == expected.pane())
        else {
            // The pane's record is gone since the paint — the region is
            // stale; the press drops rather than chase a position.
            return PointerOutcome::Ignored;
        };
        if leaf.record != expected {
            // Re-minted since the paint: the click belongs to the dead
            // incarnation and never migrates to the replacement.
            return PointerOutcome::Ignored;
        }
        let inside_content = contains(content, event.column, event.row);
        if probe.focus.as_ref() == Some(&expected) {
            if !inside_content {
                return PointerOutcome::Ignored;
            }
            return self.start_focused_press(
                view,
                port,
                probe,
                now,
                event,
                PaneTarget { expected, content },
            );
        }
        // Local focus is a read-side operation. An attached ReadOnly leaf
        // can therefore become the exact focus for selection even though it
        // can never accept application input.
        if !leaf.attached {
            return PointerOutcome::Ignored;
        }
        // Focus the pane before anything reaches it. The panel identity is
        // stable, never positional; the session plans a *local* focus
        // transition for the already-attached sibling — no attach or
        // detach round trip is minted for a click.
        let disposition = port.apply_client_action(
            (ClientAction::FocusPanel(expected.pane().panel().clone())).into(),
        );
        if let Some(disposition) = disposition {
            view.commit_pointer(disposition);
        }
        if !inside_content {
            // Title-row presses focus and stop — the pane sees nothing.
            return PointerOutcome::RegionsChanged;
        }
        // Adjudicate against a fresh probe: the local transition installs
        // the desired pane synchronously, so the post-press facts are the
        // verdict. `desired` landing on the recorded logical pane means
        // the focus was accepted; the press then still waits on the
        // *exact* recorded incarnation becoming the live focus — a
        // re-minted record of the same `(anchor, panel)` never substitutes.
        let after = port.pointer_probe();
        if after.desired.as_ref() != Some(expected.pane()) {
            // Focus refused (busy, departing, or redirected): a dead click
            // that queues nothing.
            return PointerOutcome::RegionsChanged;
        }
        if after.focus.as_ref() == Some(&expected) {
            let ready = after
                .panes
                .iter()
                .find(|pane| pane.record == expected)
                .is_some_and(|pane| {
                    pane.controlling.is_some()
                        && pane.frame_available
                        && (after.input_ready || after.selection_ready)
                });
            if after.frame.filter(|_| ready).is_none() {
                let mut queued = VecDeque::new();
                queued.push_back(event);
                self.gesture = Gesture::Pending(PendingFocus {
                    button: event.button,
                    expected,
                    viewport: content,
                    pressed_at: now,
                    deadline: now + PENDING_FOCUS_TIMEOUT,
                    queued,
                });
                return PointerOutcome::RegionsChanged;
            }
            return self.start_focused_press(
                view,
                port,
                &after,
                now,
                event,
                PaneTarget { expected, content },
            );
        }
        // The focus was accepted but has not reached the recorded
        // incarnation — the record is still attaching, or the incarnation
        // changed between paint and press. Queue until the fresh probe's
        // verdict resolves which.
        let mut queued = VecDeque::new();
        queued.push_back(event);
        self.gesture = Gesture::Pending(PendingFocus {
            button: event.button,
            expected,
            viewport: content,
            pressed_at: now,
            deadline: now + PENDING_FOCUS_TIMEOUT,
            queued,
        });
        PointerOutcome::RegionsChanged
    }

    /// Wheel over a pane region outside any gesture: tracking panes get a
    /// report; a non-reporting alternate-screen pane gets cursor arrows
    /// through the bounded stream; untracked primary content uses history.
    fn wheel_on_pane<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        event: MouseEvent,
        target: &PaneTarget,
    ) -> PointerOutcome {
        if event.mods.shift {
            return PointerOutcome::Ignored;
        }
        if probe.focus.as_ref() != Some(&target.expected) {
            // Read-side history belongs to the pane under the pointer. The
            // native owner rechecks its current screen and mouse contract;
            // this path never invokes focus, control, or program input.
            if matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
                && contains(target.content, event.column, event.row)
                && probe.panes.iter().any(|pane| {
                    pane.record == target.expected && pane.attached && pane.frame_available
                })
                && port.pane_input(PaneInput::HistoryWheel {
                    pane: &target.expected,
                    up: event.button == MouseButton::WheelUp,
                }) == PaneAdmission::Admitted
            {
                return PointerOutcome::Handled;
            }
            return PointerOutcome::Ignored;
        }
        if !contains(target.content, event.column, event.row) {
            return if probe.viewport_non_live {
                PointerOutcome::Handled
            } else {
                PointerOutcome::Ignored
            };
        }
        let vertical = matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown);
        // A non-Live focus core owns the wheel before any frame contract:
        // each notch scrolls its retained window locally, three rows per
        // notch, and nothing encodes to the pane.
        if probe.viewport_non_live {
            if !vertical {
                return PointerOutcome::Ignored;
            }
            // A non-Live focus owns the wheel outright: local scroll when
            // the core accepts it, silent consumption otherwise — never an
            // encoded byte, never an overlay dismissal.
            let _admission = port.pane_input(PaneInput::HistoryWheel {
                pane: &target.expected,
                up: event.button == MouseButton::WheelUp,
            });
            return PointerOutcome::Handled;
        }
        let Some(frame) = probe.frame else {
            return PointerOutcome::Ignored;
        };
        match route_pane_pointer(
            frame.tracking,
            &event,
            frame.alternate_screen && frame.alternate_scroll,
        ) {
            PanePointerRoute::ProgramReport => {
                let mut gesture = PaneGesture {
                    button: event.button,
                    expected: target.expected.clone(),
                    viewport: target.content,
                    pressed: false,
                    last_cell: None,
                };
                gesture.forward(view, port, &frame, event);
                PointerOutcome::Handled
            }
            PanePointerRoute::AlternateScroll => {
                self.wheel.accumulate(
                    event.button == MouseButton::WheelUp,
                    usize::from(target.content.height),
                    frame.application_cursor,
                );
                if self.flush_wheel(view, port) {
                    PointerOutcome::Handled
                } else {
                    PointerOutcome::Ignored
                }
            }
            PanePointerRoute::Client if !frame.alternate_screen && vertical => {
                let _admission = port.pane_input(PaneInput::HistoryWheel {
                    pane: &target.expected,
                    up: event.button == MouseButton::WheelUp,
                });
                PointerOutcome::Handled
            }
            PanePointerRoute::Client | PanePointerRoute::Host => PointerOutcome::Ignored,
        }
    }

    /// An event while a pane gesture owns the pointer: the press's region
    /// still owns it — a drag leaving the viewport keeps encoding against
    /// the same origin, and only the release ends the gesture.
    fn pane_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        event: MouseEvent,
    ) -> PointerOutcome {
        let Gesture::Pane(gesture) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        if !gesture.alive(probe) {
            self.gesture = Gesture::Idle;
            return PointerOutcome::Ignored;
        }
        if !gesture.accepts(event) {
            return PointerOutcome::Handled;
        }
        if probe.viewport_non_live {
            // The pane entered history mid-gesture: remaining events are
            // consumed locally; only the release still ends the gesture.
            if event.action == MouseAction::Release {
                self.gesture = Gesture::Idle;
            }
            return PointerOutcome::Handled;
        }
        if event.mods.shift {
            // Shift hands the gesture to host selection mid-drag: the
            // event is dropped but the gesture survives so an unshifted
            // release still reaches the pane.
            return PointerOutcome::Handled;
        }
        let Some(frame) = probe.frame else {
            self.gesture = Gesture::Idle;
            return PointerOutcome::Ignored;
        };
        // The press-time owner retains every accepted event. Encoding may
        // be silent after a tracking-mode change, but the matching release
        // still clears this gesture instead of becoming fresh Client input.
        gesture.forward(view, port, &frame, event);
        if event.action == MouseAction::Release {
            self.gesture = Gesture::Idle;
        }
        PointerOutcome::Handled
    }

    /// An event while a pane press waits for its focus switch: readiness
    /// is re-evaluated against a fresh probe, then the event queues behind
    /// the press — motion merges so the queue stays small.
    fn pending_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        now: Instant,
        event: MouseEvent,
    ) -> PointerOutcome {
        match self.pending_status(probe, now) {
            PendingStatus::Ready => {
                self.settle_pending(view, port, probe, now);
                // The gesture either became a pane gesture or resolved
                // away — re-route this event through the normal path.
                return match self.gesture {
                    Gesture::Pane(_) => self.pane_event(view, port, probe, event),
                    Gesture::Selection(_) => self.selection_event(port, probe, event),
                    _ => self.fresh_event(view, port, probe, now, event),
                };
            }
            PendingStatus::Waiting => {}
            PendingStatus::Orphaned => {
                self.gesture = Gesture::Idle;
                return PointerOutcome::Ignored;
            }
            PendingStatus::Expired => {
                self.gesture = Gesture::Idle;
                view.report_pointer_input_dropped();
                return PointerOutcome::Handled;
            }
        }
        let completed = matches!(&self.gesture, Gesture::Pending(pending)
        if pending.queued.back().is_some_and(|last| {
            last.action == MouseAction::Release
                && (last.button == pending.button || last.button == MouseButton::None)
        }));
        let replacement = if completed && plain_primary_press(event) {
            let expected = match &self.gesture {
                Gesture::Pending(pending) => Some(&pending.expected),
                _ => None,
            };
            self.map
                .as_ref()
                .and_then(|map| match map.hit(event.column, event.row) {
                    HitResolution::Region(HitTarget::Pane { pane, content })
                        if expected == Some(pane)
                            && contains(*content, event.column, event.row) =>
                    {
                        Some(*content)
                    }
                    _ => None,
                })
        } else {
            None
        };
        let Gesture::Pending(pending) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        // Queue overflow cancels before a replacement can admit fresh input.
        if pending.queued.len() >= PENDING_FOCUS_LIMIT {
            self.gesture = Gesture::Idle;
            view.report_pointer_input_dropped();
            return PointerOutcome::Handled;
        }
        if let Some(viewport) = replacement {
            // A completed click can be replaced within the original focus wait.
            pending.button = event.button;
            pending.viewport = viewport;
            pending.pressed_at = now;
            pending.queued.clear();
            pending.queued.push_back(event);
            return PointerOutcome::Handled;
        }
        if event.action == MouseAction::Motion
            && let Some(last) = pending.queued.back_mut()
            && last.action == MouseAction::Motion
        {
            // Motion merges: only the newest cell matters for a drag that
            // has not started reporting yet.
            *last = event;
            return PointerOutcome::Handled;
        }
        pending.queued.push_back(event);
        PointerOutcome::Handled
    }

    /// Readiness verdict for the pending gesture, evaluated against a
    /// fresh probe. Admission is the live facts, never positional or a
    /// generation delta: the probe must still know the *recorded* exact
    /// incarnation — a record re-minted for the same `(anchor, panel)`
    /// fails the ref equality and orphans the queue rather than migrating
    /// the press onto the replacement. Readiness then mirrors `forward`'s
    /// own gate — Ready phase under a Controlling mode with the recorded
    /// incarnation as the live focus — plus a usable frame of that same
    /// incarnation. ReadOnly is a Ready read-side verdict for selection;
    /// application input keeps its independent `input_ready` gate.
    fn pending_status(&self, probe: &PointerProbe, now: Instant) -> PendingStatus {
        let Gesture::Pending(pending) = &self.gesture else {
            return PendingStatus::Orphaned;
        };
        let Some(pane) = probe
            .panes
            .iter()
            .find(|pane| pane.record.pane() == pending.expected.pane())
        else {
            // No record for the logical pane at all — it is gone.
            return PendingStatus::Orphaned;
        };
        if pending.button == MouseButton::Right
            && pane.right_click != orchbee_workbench_protocol::PanelRightClickTarget::Panel
        {
            return PendingStatus::Orphaned;
        }
        if pane.record != pending.expected {
            // The record was re-minted since the paint — a fresh
            // incarnation of the same logical pane is a different target;
            // the queued press never migrates onto it.
            return PendingStatus::Orphaned;
        }
        if pane.attached {
            if probe.focus.as_ref() != Some(&pending.expected) {
                // Attached and settled with the focus elsewhere — the
                // local focus was refused or redirected; never chase.
                return PendingStatus::Orphaned;
            }
            // The mode must have settled, but ReadOnly is sufficient for a
            // Host-backed selection. A tracking pane will independently
            // consult `input_ready` when this queue is adjudicated.
            if pane.controlling.is_some()
                && pane.frame_available
                && probe.frame.is_some()
                && (probe.input_ready || probe.selection_ready)
            {
                return PendingStatus::Ready;
            }
            // Attached and focused but the phase is still converging or
            // the first frame has not landed — wait inside the same
            // bounded deadline.
        } else if probe.focus.is_some() && probe.focus.as_ref() != Some(&pending.expected) {
            // The record is still unattached while the focus already
            // settled on another live pane — cancel rather than chase.
            return PendingStatus::Orphaned;
        }
        if now >= pending.deadline {
            return PendingStatus::Expired;
        }
        PendingStatus::Waiting
    }

    /// Applies a pending verdict when one is known: Ready flushes the
    /// queued input through the now-current frame, the terminal verdicts
    /// cancel with the appropriate notice.
    fn settle_pending<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        probe: &PointerProbe,
        now: Instant,
    ) -> bool {
        match self.pending_status(probe, now) {
            PendingStatus::Waiting => return false,
            PendingStatus::Orphaned => {
                self.gesture = Gesture::Idle;
                return false;
            }
            PendingStatus::Expired => {
                self.gesture = Gesture::Idle;
                view.report_pointer_input_dropped();
                return true;
            }
            PendingStatus::Ready => {}
        }
        let Gesture::Pending(pending) = std::mem::replace(&mut self.gesture, Gesture::Idle) else {
            return false;
        };
        let Some(frame) = probe.frame else {
            return false;
        };
        let mut queued = pending.queued.into_iter();
        let Some(press) = queued.next() else {
            return true;
        };
        match route_pane_pointer(
            frame.tracking,
            &press,
            frame.alternate_screen && frame.alternate_scroll,
        ) {
            PanePointerRoute::ProgramReport => {
                if probe.viewport_non_live {
                    return true;
                }
                if !pane_input_ready(probe, &pending.expected) {
                    view.report_pointer_input_dropped();
                    return true;
                }
                let mut forwarder = PaneGesture {
                    button: pending.button,
                    expected: pending.expected,
                    viewport: pending.viewport,
                    pressed: false,
                    last_cell: None,
                };
                let mut released = false;
                for event in std::iter::once(press).chain(queued) {
                    if !forwarder.accepts(event) {
                        continue;
                    }
                    forwarder.forward(view, port, &frame, event);
                    if event.action == MouseAction::Release {
                        released = true;
                        break;
                    }
                }
                if !released && forwarder.pressed && forwarder.alive(probe) {
                    self.gesture = Gesture::Pane(forwarder);
                }
            }
            PanePointerRoute::Client if plain_primary_press(press) && probe.selection_ready => {
                if self.begin_selection(
                    port,
                    pending.expected,
                    pending.viewport,
                    pending.pressed_at,
                    press,
                ) {
                    for event in queued {
                        if !matches!(self.gesture, Gesture::Selection(_)) {
                            break;
                        }
                        self.selection_event(port, probe, event);
                    }
                }
            }
            PanePointerRoute::AlternateScroll
            | PanePointerRoute::Client
            | PanePointerRoute::Host => {}
        }
        true
    }
}

/// Crate-internal `PointerState` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait PointerStateTestAccess {
    fn projection(&self) -> PointerProjection;
    fn set_motion_observability(&mut self, observability: MotionObservability);
    fn take_projection_redraw(&mut self) -> bool;
    fn set_capture(&mut self, enabled: bool);
    fn publish(&mut self, map: FrameHitMap);
    fn invalidate_map_with_port<Port: TerminalInputPort + ?Sized>(&mut self, port: &mut Port);
    fn event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        event: MouseEvent,
    ) -> PointerOutcome;
}

#[cfg(any(test, feature = "test-support"))]
impl PointerStateTestAccess for PointerState {
    fn projection(&self) -> PointerProjection {
        PointerState::projection(self)
    }
    fn set_motion_observability(&mut self, observability: MotionObservability) {
        PointerState::set_motion_observability(self, observability);
    }
    fn take_projection_redraw(&mut self) -> bool {
        PointerState::take_projection_redraw(self)
    }
    fn set_capture(&mut self, enabled: bool) {
        PointerState::set_capture(self, enabled);
    }
    fn publish(&mut self, map: FrameHitMap) {
        PointerState::publish(self, map);
    }
    fn invalidate_map_with_port<Port: TerminalInputPort + ?Sized>(&mut self, port: &mut Port) {
        PointerState::invalidate_map_with_port(self, port);
    }
    fn event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
        event: MouseEvent,
    ) -> PointerOutcome {
        PointerState::event(self, view, port, now, event)
    }
}

#[cfg(test)]
mod tests;
