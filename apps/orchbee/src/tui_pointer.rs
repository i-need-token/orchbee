//! Pointer ownership state for the native shell.
//!
//! Three contracts live here, matching `docs/tui/mouse-input.md`:
//!
//! - The **hit map** the renderer records while painting (`HitMapBuilder`)
//!   and the shell publishes only after a complete successful draw
//!   (`FrameHitMap`). Pointer events resolve against the last published
//!   frame's regions — never a predicted layout — and regions carry stable
//!   identities, re-resolved to positions at click time via the port's
//!   `PointerProbe`.
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
use orchbee_workbench_protocol::WorkbenchTabId;

use crate::client_view::{ClientAction, ClientView};
use crate::terminal_input::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PointerFrameFacts,
    PointerOwner, PointerProbe, TerminalInputPort, encode_cursor_arrows, encode_pane_report,
    forward_or_report, pane_owns_pointer,
};
use crate::terminal_session::NativeContextLocation;
use crate::terminal_session::state::PaneRef;

/// Longest a queued pane press waits for its focus switch to install.
/// Bounded so a stuck attach cannot hold input hostage.
const PENDING_FOCUS_TIMEOUT: Duration = Duration::from_secs(2);

/// Bounded pending-focus queue: presses, motions, and wheel notches that
/// arrived while the target pane's switch installed. Overflow cancels the
/// gesture before accepting more input, with a user-facing notice.
const PENDING_FOCUS_LIMIT: usize = 16;

/// Cursor-key presses emitted per wheel notch under the alternate-screen
/// fallback — the pager-friendly amount the mouse standard fixes.
const ARROWS_PER_NOTCH: usize = 3;

/// Layers are tested front to back; regions never overlap within one
/// layer. A capturing overlay is tested before the layer beneath it, and a
/// press that hits nothing inside it dismisses instead of falling through.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HitLayer {
    /// Actionable overlay controls — navigator rows. Tested first.
    Overlay,
    /// The painted overlay body itself: inside the overlay but off every
    /// control — the click is swallowed without falling through.
    OverlaySurface,
    /// Sidebar controls remain above panes when height undocks the sidebar.
    #[cfg(windows)]
    Chrome,
    #[cfg(windows)]
    ChromeSurface,
    /// Chrome and pane regions painted under any overlay: tab-strip
    /// tokens, pane leafs.
    Foreground,
}

/// What a recorded region resolves to at click time. Regions carry stable
/// identities, never positions: the probe re-resolves them against current
/// state so a reorder between paint and click cannot misroute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HitTarget {
    /// A compact tab-strip token or a navigator tab row.
    Tab(WorkbenchTabId),
    /// A navigator context row.
    Context(NativeContextLocation),
    /// A painted pane leaf. `pane` is the exact incarnation the painter
    /// drew — anchor generation, stable panel, incarnation — so a press
    /// can never migrate onto a re-minted record of the same logical
    /// pane. `content` is its cell viewport — the region a pane program
    /// owns while it reports the pointer; the title row outside it is
    /// Client chrome.
    Pane { pane: PaneRef, content: Rect },
    /// Exact painted incarnation and stable destination within that Panel.
    Presentation {
        pane: PaneRef,
        presentation: orchbee_workbench_protocol::TerminalPresentationId,
    },
    /// Empty members carry exact structural identity and no runtime incarnation.
    Unbound {
        selected: crate::terminal_input::PointerUnboundPane,
        presentation: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    },
    /// Painted overlay body with no action — swallows the click so it
    /// cannot reach the layer beneath.
    OverlaySurface,
    /// An Agent board row — carries the row's stable session identity so
    /// a later click resolves against current rows, never a position.
    AgentBoardRow(orchbee_agent_observation_protocol::AgentSessionId),
    /// The Agent board's painted body outside its rows — swallows the
    /// press, and a wheel notch scrolls the row window.
    AgentBoardSurface,
    #[cfg(windows)]
    MachineWorkspaceRow {
        selected: crate::machine_picker::Selection,
        window: crate::machine_picker::SidebarWindow,
    },
    #[cfg(windows)]
    MachineWorkspaceSurface(crate::machine_picker::SidebarWindow),
    #[cfg(windows)]
    OpenMachineWorkspaces,
    #[cfg(windows)]
    MachineAgentRow {
        selected: crate::machine_picker::Selection,
        window: crate::machine_picker::SidebarWindow,
    },
    #[cfg(windows)]
    MachineAgentSurface(crate::machine_picker::SidebarWindow),
    #[cfg(windows)]
    OpenMachineAgents,
}

#[derive(Clone, Debug)]
struct HitRegion {
    layer: HitLayer,
    rect: Rect,
    target: HitTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HitResolution<'a> {
    Region(&'a HitTarget),
    /// A capturing overlay was active and the cell hit nothing — an
    /// outside click: dismiss, never fall through.
    OverlayBackdrop,
    /// Nothing claimed the cell.
    Dead,
}

fn contains(rect: Rect, column: u16, row: u16) -> bool {
    u32::from(column) >= u32::from(rect.x)
        && u32::from(column) < u32::from(rect.x) + u32::from(rect.width)
        && u32::from(row) >= u32::from(rect.y)
        && u32::from(row) < u32::from(rect.y) + u32::from(rect.height)
}

/// The hit map of one successfully drawn frame. The renderer builds it
/// staged; the shell publishes it only when the draw completes — between
/// a layout change and the next draw there is no map and pointer events
/// drop rather than misroute.
#[derive(Clone, Debug, Default)]
pub(crate) struct FrameHitMap {
    overlay_active: bool,
    regions: Vec<HitRegion>,
}

impl FrameHitMap {
    fn hit(&self, column: u16, row: u16) -> HitResolution<'_> {
        if self.overlay_active {
            // A capturing overlay owns the press domain end to end: only
            // its own rows and painted surface resolve, controls before
            // surface. Every other cell — including foreground chrome the
            // overlay does not cover — is a dismiss-only backdrop, never a
            // fall-through to the layer below.
            for layer in [HitLayer::Overlay, HitLayer::OverlaySurface] {
                for region in &self.regions {
                    if region.layer == layer && contains(region.rect, column, row) {
                        return HitResolution::Region(&region.target);
                    }
                }
            }
            return HitResolution::OverlayBackdrop;
        }
        for layer in [
            #[cfg(windows)]
            HitLayer::Chrome,
            #[cfg(windows)]
            HitLayer::ChromeSurface,
            HitLayer::Foreground,
        ] {
            for region in &self.regions {
                if region.layer == layer && contains(region.rect, column, row) {
                    return HitResolution::Region(&region.target);
                }
            }
        }
        HitResolution::Dead
    }

    /// Test-only probe: the target a press at `(column, row)` would
    /// resolve to — the drawn-set witness that a zoomed frame exposes
    /// exactly the focused pane and nothing else.
    #[cfg(test)]
    pub(crate) fn hit_target_at(&self, column: u16, row: u16) -> Option<&HitTarget> {
        match self.hit(column, row) {
            HitResolution::Region(target) => Some(target),
            HitResolution::OverlayBackdrop | HitResolution::Dead => None,
        }
    }
}

/// The renderer's staged recorder — same geometry the paint used, because
/// each painter records the rect it actually drew.
#[derive(Default)]
pub(crate) struct HitMapBuilder {
    map: FrameHitMap,
}

impl HitMapBuilder {
    pub(crate) const fn new() -> Self {
        Self {
            map: FrameHitMap {
                overlay_active: false,
                regions: Vec::new(),
            },
        }
    }

    /// A capturing overlay is open this frame: an unresolved press is an
    /// outside click that dismisses it.
    pub(crate) fn mark_overlay_active(&mut self) {
        self.map.overlay_active = true;
    }

    pub(crate) fn record(&mut self, layer: HitLayer, rect: Rect, target: HitTarget) {
        if rect.is_empty() {
            return;
        }
        self.map.regions.push(HitRegion {
            layer,
            rect,
            target,
        });
    }

    /// Exact positive-size pane content recorded by this paint pass.
    #[cfg(windows)]
    pub(crate) fn painted_panes(&self) -> impl Iterator<Item = &PaneRef> {
        self.map
            .regions
            .iter()
            .filter_map(|region| match &region.target {
                HitTarget::Pane { pane, content }
                    if region.layer == HitLayer::Foreground
                        && !region.rect.intersection(*content).is_empty() =>
                {
                    Some(pane)
                }
                _ => None,
            })
    }

    pub(crate) fn finish(self) -> FrameHitMap {
        self.map
    }
}

/// The wheel-fallback accumulator: pending arrow presses in one
/// direction. A reversal clears the stream rather than mixing directions
/// into one flush; the flush emits at most one viewport of arrows so a
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
            // A reversal mid-burst clears the queued remainder and drops
            // this notch — a direction change never flushes a mixed burst.
            self.pending = 0;
            self.emitted = 0;
        } else {
            self.pending = self.pending.saturating_add(ARROWS_PER_NOTCH);
        }
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
    deadline: Instant,
    queued: VecDeque<MouseEvent>,
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
    Pending(PendingFocus),
    Pane(PaneGesture),
}

/// What one host report did to the pointer state — the dispatcher maps it
/// to `after_layout` work and map invalidation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PointerOutcome {
    /// Not ours or swallowed — the packet moves on untouched.
    Ignored,
    /// Consumed without repainting regions: pane-bound bytes, a queued
    /// event, a wheel notch. The recorded map stays authoritative.
    Handled,
    /// Consumed and painted regions may differ (navigation applied, an
    /// overlay dismissed): the map is stale until the next draw. The
    /// gesture the event belongs to survives — release/motion follow it.
    RegionsChanged,
    /// A sidebar navigation owns the remainder of its host input packet.
    #[cfg(windows)]
    MachineNavigation,
}

#[cfg(test)]
impl PointerOutcome {
    /// The event was consumed — the dispatcher runs the viewport-sync
    /// path. `Ignored` is the only outcome that skips it.
    pub(crate) const fn handled(self) -> bool {
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
    /// The settled attachment is ReadOnly — adjudicate negatively.
    ReadOnly,
}

/// Pointer ownership state owned by the dispatcher: the capture flag, the
/// last published hit map, the gesture lifecycle, and the wheel stream.
/// `Default` is released — the shell settles the armed state only after
/// the terminal mode write succeeds.
#[derive(Default)]
pub(crate) struct PointerState {
    capture: bool,
    map: Option<FrameHitMap>,
    gesture: Gesture,
    wheel: WheelStream,
}

impl PointerState {
    pub(crate) const fn capture_enabled(&self) -> bool {
        self.capture
    }

    /// The lifecycle write succeeded — the flag mirrors the armed state.
    /// Releasing is immediate: in-flight gestures and the wheel stream
    /// die so nothing reaches a pane after the host stops reporting.
    pub(crate) fn set_capture(&mut self, enabled: bool) {
        if self.capture == enabled {
            return;
        }
        self.capture = enabled;
        if !enabled {
            self.gesture = Gesture::Idle;
            self.wheel.clear();
        }
    }

    /// The draw completed — the recorded map becomes the resolution
    /// authority until the next layout change.
    pub(crate) fn publish(&mut self, map: FrameHitMap) {
        self.map = Some(map);
    }

    /// The layout changed outside the pointer path — a resize, a
    /// keyboard-driven transition, a half-painted draw: the recorded
    /// regions no longer describe the screen, an in-flight gesture holds
    /// press-time geometry that no longer matches, and the wheel stream
    /// holds the old viewport bound. All three cancel; between now and
    /// the next publish every pointer event drops.
    pub(crate) fn invalidate_map(&mut self) {
        self.map = None;
        self.gesture = Gesture::Idle;
        self.wheel.clear();
    }

    /// A handled pointer event repainted regions (navigation applied or
    /// an overlay dismissed): the map stales until the next draw, but the
    /// gesture the event belongs to keeps owning release/motion — a
    /// release is never dropped mid-gesture. The dispatcher calls this on
    /// `RegionsChanged`; the map itself never decides.
    pub(crate) fn stale_map(&mut self) {
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
        if !self.capture {
            return PointerOutcome::Ignored;
        }
        let probe = port.pointer_probe();
        if probe.input_captured {
            // A modal draft or an armed confirmation owns input: pointer
            // events are swallowed without producing gestures.
            self.gesture = Gesture::Idle;
            self.wheel.clear();
            #[cfg(windows)]
            view.machines.cancel_sidebar_scroll();
            return PointerOutcome::Ignored;
        }
        match self.gesture {
            Gesture::Pending(_) => self.pending_event(view, port, &probe, now, event),
            Gesture::Pane(_) => self.pane_event(view, port, &probe, event),
            Gesture::Idle => self.fresh_event(view, port, &probe, now, event),
        }
    }

    /// Advances pending-focus readiness and the wheel flush between
    /// packets and on idle passes. Returns `true` when bytes moved or a
    /// gesture settled — the caller redraws.
    pub(crate) fn tick<Port: TerminalInputPort + ?Sized>(
        &mut self,
        view: &mut ClientView,
        port: &mut Port,
        now: Instant,
    ) -> bool {
        let probe = port.pointer_probe();
        #[cfg(windows)]
        if !self.capture
            || probe.input_captured
            || view
                .overlay()
                .is_some_and(|o| o != crate::client_view::Overlay::BusinessSidebar)
        {
            view.machines.cancel_sidebar_scroll();
        }
        if probe.input_captured {
            let changed = !matches!(self.gesture, Gesture::Idle) || self.wheel.pending != 0;
            self.gesture = Gesture::Idle;
            self.wheel.clear();
            return changed;
        }
        let mut changed = false;
        #[cfg(windows)]
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
        let Some(map) = self.map.as_ref() else {
            return PointerOutcome::Ignored;
        };
        match map.hit(event.column, event.row) {
            HitResolution::Dead => PointerOutcome::Ignored,
            HitResolution::OverlayBackdrop => {
                // An outside click on a capturing overlay dismisses it and
                // dies here — nothing reaches the regions beneath.
                if event.action == MouseAction::Press {
                    view.dismiss_overlay();
                    // The board's own wheel guard: an outside wheel notch
                    // is swallowed whole — only an ordinary outside Press
                    // dismisses the board. Other overlays keep their
                    // existing backdrop semantics above.
                    if !matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown) {
                        port.agent_board_dismiss();
                    }
                    return PointerOutcome::RegionsChanged;
                }
                PointerOutcome::Ignored
            }
            HitResolution::Region(HitTarget::OverlaySurface) => PointerOutcome::Ignored,
            #[cfg(windows)]
            HitResolution::Region(HitTarget::OpenMachineWorkspaces) => {
                if event.action == MouseAction::Press
                    && event.button == MouseButton::Left
                    && event.mods == crate::client_view::Modifiers::NONE
                    && view.machines.available
                {
                    view.dismiss_overlay();
                    view.machines.show_workspaces();
                    PointerOutcome::MachineNavigation
                } else {
                    PointerOutcome::Ignored
                }
            }
            #[cfg(windows)]
            HitResolution::Region(HitTarget::MachineWorkspaceRow { selected, window }) => {
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
                    MouseButton::Left if view.machines.activate_sidebar_workspace(selected) => {
                        view.dismiss_overlay();
                        PointerOutcome::MachineNavigation
                    }
                    _ => PointerOutcome::Ignored,
                }
            }
            #[cfg(windows)]
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
            #[cfg(windows)]
            HitResolution::Region(HitTarget::OpenMachineAgents) => {
                if event.action == MouseAction::Press
                    && event.button == MouseButton::Left
                    && event.mods == crate::client_view::Modifiers::NONE
                    && view.machines.available
                {
                    view.dismiss_overlay();
                    view.machines.show_agents();
                    PointerOutcome::MachineNavigation
                } else {
                    PointerOutcome::Ignored
                }
            }
            #[cfg(windows)]
            HitResolution::Region(HitTarget::MachineAgentRow { selected, window }) => {
                if event.action != MouseAction::Press
                    || event.mods != crate::client_view::Modifiers::NONE
                {
                    return PointerOutcome::Ignored;
                }
                match event.button {
                    MouseButton::WheelUp | MouseButton::WheelDown => {
                        view.machines
                            .queue_sidebar_scroll(*window, event.button == MouseButton::WheelUp);
                        PointerOutcome::Handled
                    }
                    MouseButton::Left if view.machines.activate_sidebar_agent(selected) => {
                        view.dismiss_overlay();
                        PointerOutcome::MachineNavigation
                    }
                    _ => PointerOutcome::Ignored,
                }
            }
            #[cfg(windows)]
            HitResolution::Region(HitTarget::MachineAgentSurface(window)) => {
                if event.action == MouseAction::Press
                    && event.mods == crate::client_view::Modifiers::NONE
                    && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
                {
                    view.machines
                        .queue_sidebar_scroll(*window, event.button == MouseButton::WheelUp);
                    PointerOutcome::Handled
                } else {
                    PointerOutcome::Ignored
                }
            }
            HitResolution::Region(HitTarget::AgentBoardRow(session)) => {
                if event.action != MouseAction::Press {
                    return PointerOutcome::Ignored;
                }
                match event.button {
                    MouseButton::WheelUp | MouseButton::WheelDown => {
                        // Wheel scrolls the board's row window — the map
                        // stays valid because row targets re-resolve by
                        // stable session identity, never position.
                        let _scrolled =
                            port.agent_board_scroll(event.button == MouseButton::WheelUp);
                        PointerOutcome::Handled
                    }
                    _ => {
                        // Identity re-resolution makes a stale-map click
                        // safe: the session is selected wherever it now
                        // sits, or the click dies cleanly if it left.
                        let _selected = port.agent_board_select(session);
                        PointerOutcome::Handled
                    }
                }
            }
            HitResolution::Region(HitTarget::AgentBoardSurface) => {
                if event.action == MouseAction::Press
                    && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
                {
                    let _scrolled = port.agent_board_scroll(event.button == MouseButton::WheelUp);
                    PointerOutcome::Handled
                } else {
                    PointerOutcome::Ignored
                }
            }
            HitResolution::Region(HitTarget::Tab(tab)) => {
                if event.action != MouseAction::Press {
                    return PointerOutcome::Ignored;
                }
                let Some(index) = probe.tab_index(tab) else {
                    return PointerOutcome::Ignored;
                };
                let Ok(position) = u8::try_from(probe.tabs[index].position) else {
                    return PointerOutcome::Ignored;
                };
                let disposition = port.apply_client_action(ClientAction::SelectTab(position));
                if let Some(disposition) = disposition {
                    view.commit_pointer(disposition);
                }
                PointerOutcome::RegionsChanged
            }
            HitResolution::Region(HitTarget::Context(location)) => {
                if event.action != MouseAction::Press {
                    return PointerOutcome::Ignored;
                }
                let Some(index) = probe.context_index(location) else {
                    return PointerOutcome::Ignored;
                };
                let disposition = port.apply_client_action(ClientAction::SelectContextAt(index));
                if let Some(disposition) = disposition {
                    view.commit_pointer(disposition);
                }
                PointerOutcome::RegionsChanged
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
                {
                    return PointerOutcome::Ignored;
                }
                let action = match presentation {
                    Some(id) => ClientAction::SelectPresentation {
                        panel: selected.key.panel().clone(),
                        presentation: id.clone(),
                    },
                    None => ClientAction::FocusPanel(selected.key.panel().clone()),
                };
                if let Some(disposition) = port.apply_client_action(action) {
                    view.commit_pointer(disposition);
                }
                PointerOutcome::RegionsChanged
            }
            HitResolution::Region(HitTarget::Presentation { pane, presentation }) => {
                if event.action != MouseAction::Press
                    || event.button != MouseButton::Left
                    || event.mods != crate::client_view::Modifiers::NONE
                    || probe
                        .desired
                        .as_ref()
                        .is_none_or(|desired| desired.anchor() != pane.pane().anchor())
                    || !probe.panes.iter().any(|live| &live.record == pane)
                {
                    return PointerOutcome::Ignored;
                }
                let disposition = port.apply_client_action(ClientAction::SelectPresentation {
                    panel: pane.pane().panel().clone(),
                    presentation: presentation.clone(),
                });
                if let Some(disposition) = disposition {
                    view.commit_pointer(disposition);
                }
                PointerOutcome::RegionsChanged
            }
            HitResolution::Region(HitTarget::Pane { pane, content }) => {
                let target = PaneTarget {
                    expected: pane.clone(),
                    content: *content,
                };
                self.pane_press(view, port, probe, now, event, target)
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
            && matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown)
        {
            return self.wheel_on_pane(view, port, probe, event, &PaneTarget { expected, content });
        }
        if event.action != MouseAction::Press {
            // A non-Live focus owns every non-wheel pointer event locally:
            // motion and release inside its content die here just like the
            // press did — they mint no gesture and encode no bytes.
            if probe.focus.as_ref() == Some(&expected)
                && probe.viewport_non_live
                && probe.frame.is_some()
                && contains(content, event.column, event.row)
            {
                return PointerOutcome::Handled;
            }
            return PointerOutcome::Ignored;
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
            let disposition = port.apply_client_action(ClientAction::SelectTab(position));
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
            // Already focused: the press belongs to the pane only inside
            // its viewport while it reports the pointer. The title row and
            // non-reporting panes are Client territory.
            if !inside_content {
                return PointerOutcome::Ignored;
            }
            let Some(frame) = probe.frame else {
                return PointerOutcome::Ignored;
            };
            if probe.viewport_non_live {
                // The focused pane's history core is non-Live: every
                // non-wheel pointer event belongs to the client overlay,
                // never to application mouse bytes.
                return PointerOutcome::Handled;
            }
            return match pane_owns_pointer(
                frame.tracking,
                &event,
                frame.alternate_screen && frame.alternate_scroll,
            ) {
                PointerOwner::Pane => {
                    self.gesture = Gesture::Pane(PaneGesture {
                        button: event.button,
                        expected,
                        viewport: content,
                        pressed: false,
                        last_cell: None,
                    });
                    let Gesture::Pane(gesture) = &mut self.gesture else {
                        return PointerOutcome::Ignored;
                    };
                    gesture.forward(view, port, &frame, event);
                    PointerOutcome::Handled
                }
                PointerOwner::Client | PointerOwner::Host => PointerOutcome::Ignored,
            };
        }
        // A pane without a live attachment cannot take a local focus — the
        // planner refuses it anyway — and a leaf that cannot currently
        // admit input for this client must never let a click steal the
        // focus from wherever it rests. That covers the settled ReadOnly
        // verdict and the still-unsettled `None` mode between Attached
        // and SetupConfigured: the title row stays Client territory and
        // drops quietly, while a content press reports the same negative
        // adjudication the pending ReadOnly branch publishes — a factual
        // input drop, not a permission diagnosis.
        let not_controlling = !matches!(leaf.controlling, Some(true));
        if !leaf.attached || (not_controlling && !inside_content) {
            return PointerOutcome::Ignored;
        }
        if not_controlling {
            view.report_pointer_input_dropped();
            return PointerOutcome::Handled;
        }
        // Focus the pane before anything reaches it. The panel identity is
        // stable, never positional; the session plans a *local* focus
        // transition for the already-attached sibling — no attach or
        // detach round trip is minted for a click.
        let disposition =
            port.apply_client_action(ClientAction::FocusPanel(expected.pane().panel().clone()));
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
            // Focus settled on the exact recorded incarnation. A usable
            // current frame settles immediately; without one the press
            // queues until the projection's first frame lands.
            let Some(frame) = after.frame else {
                let mut queued = VecDeque::new();
                queued.push_back(event);
                self.gesture = Gesture::Pending(PendingFocus {
                    button: event.button,
                    expected,
                    viewport: content,
                    deadline: now + PENDING_FOCUS_TIMEOUT,
                    queued,
                });
                return PointerOutcome::RegionsChanged;
            };
            if !after.viewport_non_live
                && let PointerOwner::Pane = pane_owns_pointer(
                    frame.tracking,
                    &event,
                    frame.alternate_screen && frame.alternate_scroll,
                )
            {
                self.gesture = Gesture::Pane(PaneGesture {
                    button: event.button,
                    expected,
                    viewport: content,
                    pressed: false,
                    last_cell: None,
                });
                let Gesture::Pane(gesture) = &mut self.gesture else {
                    return PointerOutcome::Handled;
                };
                gesture.forward(view, port, &frame, event);
            }
            return PointerOutcome::Handled;
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
        if probe.focus.as_ref() != Some(&target.expected) {
            // Wheel never focuses: only the focused pane's contract
            // applies, and only while the *recorded* incarnation still
            // owns the focus. A wheel over an unfocused pane — or over a
            // re-minted record — is a dead notch.
            return PointerOutcome::Ignored;
        }
        if event.mods.shift {
            return PointerOutcome::Ignored;
        }
        if !contains(target.content, event.column, event.row) {
            return if probe.viewport_non_live {
                PointerOutcome::Handled
            } else {
                PointerOutcome::Ignored
            };
        }
        // A non-Live focus core owns the wheel before any frame contract:
        // each notch scrolls its retained window locally, three rows per
        // notch, and nothing encodes to the pane.
        if probe.viewport_non_live {
            // A non-Live focus owns the wheel outright: local scroll when
            // the core accepts it, silent consumption otherwise — never an
            // encoded byte, never an overlay dismissal.
            port.history_wheel(&target.expected, event.button == MouseButton::WheelUp);
            return PointerOutcome::Handled;
        }
        let Some(frame) = probe.frame else {
            return PointerOutcome::Ignored;
        };
        match pane_owns_pointer(
            frame.tracking,
            &event,
            frame.alternate_screen && frame.alternate_scroll,
        ) {
            PointerOwner::Pane if frame.tracking != MouseTracking::None => {
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
            PointerOwner::Pane => {
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
            PointerOwner::Client if !frame.alternate_screen => {
                port.history_wheel(&target.expected, event.button == MouseButton::WheelUp);
                PointerOutcome::Handled
            }
            PointerOwner::Client | PointerOwner::Host => PointerOutcome::Ignored,
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
                    _ => self.fresh_event(view, port, probe, now, event),
                };
            }
            PendingStatus::Waiting => {}
            PendingStatus::Orphaned => {
                self.gesture = Gesture::Idle;
                return PointerOutcome::Ignored;
            }
            PendingStatus::Expired | PendingStatus::ReadOnly => {
                self.gesture = Gesture::Idle;
                view.report_pointer_input_dropped();
                return PointerOutcome::Handled;
            }
        }
        let Gesture::Pending(pending) = &mut self.gesture else {
            return PointerOutcome::Ignored;
        };
        // Queue overflow cancels the pending gesture before accepting new
        // input — never an unbounded hold, never a silent tail drop.
        if pending.queued.len() >= PENDING_FOCUS_LIMIT {
            self.gesture = Gesture::Idle;
            view.report_pointer_input_dropped();
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
    /// incarnation. A local focus onto an already-attached sibling
    /// settles exactly the same way: the frame is there, so it is Ready.
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
            match pane.controlling {
                // A settled ReadOnly verdict cancels; `None` means the
                // controller mode has not arrived yet — keep waiting
                // inside the same bounded deadline.
                Some(false) => return PendingStatus::ReadOnly,
                // Admission mirrors `forward`'s own gate — live Ready
                // phase, Controlling mode, the recorded incarnation as
                // exact focus — plus a usable frame of that incarnation.
                _ if probe.input_ready && pane.frame_available && probe.frame.is_some() => {
                    return PendingStatus::Ready;
                }
                _ => {}
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
            PendingStatus::Expired | PendingStatus::ReadOnly => {
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
        if probe.viewport_non_live {
            // The focus settled onto a non-Live history pane: the queued
            // press and everything behind it belong to the client overlay —
            // consumed as a dead gesture, never replayed as pane bytes.
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
        for event in pending.queued {
            if event.mods.shift || !forwarder.accepts(event) {
                continue;
            }
            match pane_owns_pointer(
                frame.tracking,
                &event,
                frame.alternate_screen && frame.alternate_scroll,
            ) {
                PointerOwner::Pane if frame.tracking != MouseTracking::None => {
                    forwarder.forward(view, port, &frame, event);
                    if event.action == MouseAction::Release {
                        released = true;
                        break;
                    }
                }
                PointerOwner::Pane => {
                    // Non-reporting alternate-screen pane: wheel notches
                    // queue as cursor arrows through the bounded stream.
                    if matches!(event.button, MouseButton::WheelUp | MouseButton::WheelDown) {
                        self.wheel.accumulate(
                            event.button == MouseButton::WheelUp,
                            usize::from(pending.viewport.height),
                            frame.application_cursor,
                        );
                    }
                }
                PointerOwner::Client | PointerOwner::Host => {}
            }
        }
        // A drag that ended inside the queue does not leave a live pane
        // gesture behind; a still-held press keeps owning the pointer.
        if !released && forwarder.pressed && forwarder.alive(probe) {
            self.gesture = Gesture::Pane(forwarder);
        }
        true
    }
}

#[cfg(test)]
mod tests;
