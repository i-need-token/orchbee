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
    PointerOwner, PointerProbe, PointerSelectionAdmission, PointerSelectionCell,
    PointerSelectionIntent, TerminalInputPort, encode_cursor_arrows, encode_pane_report,
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

/// Maximum interval between two presses on the exact same pane cell that
/// promotes the second press from cell selection to word selection.
const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(350);

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
    #[cfg(windows)]
    SettingsRow {
        token: crate::native_settings::Token,
        row: crate::native_settings::Row,
    },
    #[cfg(windows)]
    ActionMenuButton,
    #[cfg(windows)]
    ActionMenuRow {
        token: crate::action_menu::MenuToken,
        row: usize,
    },
    /// A visible divider with its exact canonical and geometric source.
    #[cfg(windows)]
    Divider(crate::tui_layout::DrawnDivider),
    #[cfg(windows)]
    SidebarResize(crate::tui_layout::DrawnSidebarWidth),
    #[cfg(windows)]
    Popup {
        id: orchbee_workbench_protocol::WorkbenchRequestId,
        content: Rect,
    },
    #[cfg(windows)]
    PopupClose(orchbee_workbench_protocol::WorkbenchRequestId),
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
    MachineProjectRow {
        target: crate::machine_picker::SidebarTarget,
        window: crate::machine_picker::SidebarWindow,
    },
    #[cfg(windows)]
    SidebarDisclosure(crate::machine_picker::SidebarTarget),
    #[cfg(windows)]
    SidebarMenu(crate::machine_picker::SidebarTarget),
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

#[cfg(windows)]
impl HitTarget {
    fn sidebar_target(&self) -> Option<crate::machine_picker::SidebarTarget> {
        use crate::machine_picker::SidebarTarget;
        match self {
            Self::MachineProjectRow { target, .. }
            | Self::SidebarDisclosure(target)
            | Self::SidebarMenu(target) => Some(target.clone()),
            Self::MachineWorkspaceRow { selected, .. } => match &selected.context {
                Some(crate::terminal_session::NativeStartupTarget::Context(target)) => {
                    Some(SidebarTarget::Context {
                        machine: selected.machine.clone(),
                        target: target.clone(),
                    })
                }
                _ => None,
            },
            _ => None,
        }
    }
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
    #[cfg(windows)]
    link_sources: Vec<(Rect, crate::terminal_link_activation::PaintedLinkSource)>,
    #[cfg(windows)]
    menu_source: Option<crate::action_menu::DrawnMenuSource>,
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

    #[cfg(windows)]
    fn link_source(
        &self,
        pane: &PaneRef,
        column: u16,
        row: u16,
    ) -> Option<&crate::terminal_link_activation::PaintedLinkSource> {
        self.link_sources
            .iter()
            .find(|(rect, source)| source.pane == *pane && contains(*rect, column, row))
            .map(|(_, source)| source)
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
    /// Same-frame allocation for the Machines painter, never retained as an input hit.
    #[cfg(windows)]
    machine_indicator: Option<Rect>,
}

impl HitMapBuilder {
    #[cfg(windows)]
    pub(crate) fn start_settings(&mut self) {
        self.map.overlay_active = true;
        self.map
            .regions
            .retain(|region| !matches!(region.layer, HitLayer::Overlay | HitLayer::OverlaySurface));
    }
    #[cfg(windows)]
    pub(crate) fn record_action_menu_source(
        &mut self,
        source: Option<crate::action_menu::DrawnMenuSource>,
    ) {
        self.map.menu_source = source;
    }
    #[cfg(windows)]
    pub(crate) fn start_action_menu(&mut self) {
        self.map.overlay_active = true;
        self.map
            .regions
            .retain(|region| !matches!(region.layer, HitLayer::Overlay | HitLayer::OverlaySurface));
    }
    pub(crate) const fn new() -> Self {
        Self {
            map: FrameHitMap {
                overlay_active: false,
                regions: Vec::new(),
                #[cfg(windows)]
                link_sources: Vec::new(),
                #[cfg(windows)]
                menu_source: None,
            },
            #[cfg(windows)]
            machine_indicator: None,
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

    /// Record the link source from the same terminal paint and pane geometry
    /// as the foreground hit region. It publishes atomically with this map.
    #[cfg(windows)]
    pub(crate) fn record_link_source(
        &mut self,
        rect: Rect,
        source: crate::terminal_link_activation::PaintedLinkSource,
    ) {
        if !rect.is_empty() {
            self.map.link_sources.push((rect, source));
        }
    }

    /// A noncapturing surface covered terminal content after it was painted.
    /// Keep ordinary pane hits, but publish no terminal link source through it.
    #[cfg(windows)]
    pub(crate) fn suppress_link_sources(&mut self) {
        self.map.link_sources.clear();
    }

    #[cfg(windows)]
    pub(crate) fn record_machine_indicator(&mut self, area: Option<Rect>) {
        self.machine_indicator = area;
    }

    #[cfg(windows)]
    pub(crate) const fn machine_indicator(&self) -> Option<Rect> {
        self.machine_indicator
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

    /// Only a matching exposed divider can paint a local preview above the current frame.
    #[cfg(windows)]
    pub(crate) fn exposes_divider(&self, target: &crate::tui_layout::DrawnDivider) -> bool {
        self.map.exposes_divider(target)
    }

    #[cfg(windows)]
    pub(crate) fn exposes_sidebar_width(
        &self,
        target: &crate::tui_layout::DrawnSidebarWidth,
    ) -> bool {
        self.map.exposes_sidebar_width(target)
    }
}

#[cfg(windows)]
impl FrameHitMap {
    fn exposes_sidebar_width(&self, target: &crate::tui_layout::DrawnSidebarWidth) -> bool {
        let edge = target.edge();
        self.regions.iter().any(|region| {
            region.rect == edge
                && matches!(&region.target, HitTarget::SidebarResize(painted) if painted == target)
        }) && matches!(self.hit(edge.x, edge.y), HitResolution::Region(HitTarget::SidebarResize(painted)) if painted == target)
    }

    fn exposes_divider(&self, target: &crate::tui_layout::DrawnDivider) -> bool {
        !self.overlay_active
            && self.regions.iter().any(|region| {
                region.layer == HitLayer::Foreground
                    && region.rect == target.divider
                    && matches!(&region.target, HitTarget::Divider(painted) if painted == target)
            })
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
#[cfg(windows)]
struct LinkGesture {
    source: Option<crate::terminal_link_activation::PaintedLinkSource>,
    viewport: Rect,
    press_column: u16,
    press_row: u16,
    cell: PointerSelectionCell,
    cancelled: bool,
}

#[cfg(windows)]
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

#[cfg(windows)]
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
    Pending(PendingFocus),
    Pane(PaneGesture),
    Selection(SelectionGesture),
    #[cfg(windows)]
    Divider(crate::tui_layout::DividerDrag),
    #[cfg(windows)]
    Link(LinkGesture),
}

/// What one host report did to the pointer state — the dispatcher maps it
/// to `after_layout` work and map invalidation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum PointerOutcome {
    #[cfg(windows)]
    SidebarProjectRequested(crate::machine_picker::SidebarTarget),
    #[cfg(windows)]
    MenuRequested {
        target: crate::action_menu::MenuTarget,
        anchor: (u16, u16),
        button: MouseButton,
    },
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
pub(crate) struct PointerState {
    capture: bool,
    map: Option<FrameHitMap>,
    gesture: Gesture,
    click: Option<SelectionClick>,
    #[cfg(windows)]
    popup_gesture: Option<(
        orchbee_workbench_protocol::WorkbenchRequestId,
        Rect,
        MouseButton,
    )>,
    wheel: WheelStream,
}

impl PointerState {
    #[cfg(windows)]
    pub(crate) fn settings_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        event: MouseEvent,
    ) {
        use crate::client_view::{KeyCode, KeyEvent, Modifiers};
        if event.mods != Modifiers::NONE || event.action != MouseAction::Press {
            return;
        }
        match event.button {
            MouseButton::WheelUp | MouseButton::WheelDown => {
                port.capture_settings_key(KeyEvent::press(
                    if event.button == MouseButton::WheelUp {
                        KeyCode::Up
                    } else {
                        KeyCode::Down
                    },
                    Modifiers::NONE,
                ))
            }
            MouseButton::Left => {
                if let Some(map) = &self.map {
                    match map.hit(event.column, event.row) {
                        HitResolution::Region(HitTarget::SettingsRow { token, row }) => {
                            port.click_settings(*token, *row)
                        }
                        HitResolution::Region(HitTarget::OverlaySurface) => {}
                        _ => port
                            .capture_settings_key(KeyEvent::press(KeyCode::Esc, Modifiers::NONE)),
                    }
                }
            }
            _ => {}
        }
    }
    /// A menu cannot take over a press, pending focus or queued wheel burst.
    #[cfg(windows)]
    pub(crate) fn can_open_action_menu(&self) -> bool {
        matches!(self.gesture, Gesture::Idle)
            && self.wheel.pending == 0
            && self.popup_gesture.is_none()
    }

    /// Keyboard and pointer entries share one successfully painted row witness.
    #[cfg(windows)]
    pub(crate) fn sidebar_menu_target(
        &self,
        view: &ClientView,
        target: &crate::machine_picker::SidebarTarget,
    ) -> Option<crate::action_menu::MenuTarget> {
        use crate::{action_menu::MenuTarget, machine_picker::SidebarTarget};
        if !self.can_open_action_menu() || !view.machines.sidebar_target_exists(target) {
            return None;
        }
        let map = self.map.as_ref()?;
        let source = map.menu_source.as_ref()?;
        let exact = match target {
            SidebarTarget::Project {
                machine,
                site_id,
                project_id,
            } if machine == &view.machines.current && site_id == &source.site => {
                MenuTarget::Project(project_id.clone())
            }
            SidebarTarget::Context { machine, target }
                if machine == &view.machines.current
                    && target.site_id == source.site
                    && matches!(target.location, NativeContextLocation::Workspace { .. }) =>
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
    #[cfg(windows)]
    pub(crate) fn action_menu_event<Port: TerminalInputPort + ?Sized>(
        &mut self,
        port: &mut Port,
        event: MouseEvent,
    ) {
        if event.mods != crate::client_view::Modifiers::NONE {
            return;
        }
        if event.action != MouseAction::Press {
            return;
        }
        match event.button {
            MouseButton::WheelUp | MouseButton::WheelDown => {
                port.capture_action_menu_key(crate::client_view::KeyEvent::press(
                    if event.button == MouseButton::WheelUp {
                        crate::client_view::KeyCode::Up
                    } else {
                        crate::client_view::KeyCode::Down
                    },
                    crate::client_view::Modifiers::NONE,
                ))
            }
            MouseButton::Left => {
                if let Some(map) = &self.map {
                    match map.hit(event.column, event.row) {
                        HitResolution::Region(HitTarget::ActionMenuRow { token, row }) => {
                            port.click_action_menu(*token, *row)
                        }
                        HitResolution::Region(HitTarget::OverlaySurface) => {}
                        _ => port.cancel_action_menu(),
                    }
                }
            }
            MouseButton::Right => port.cancel_action_menu(),
            _ => {}
        }
    }
    /// Current reversible geometry, never a source of authoritative terminal dimensions.
    #[cfg(windows)]
    pub(crate) fn divider_preview(&self) -> Option<&crate::tui_layout::DividerDrag> {
        match &self.gesture {
            Gesture::Divider(drag) => Some(drag),
            _ => None,
        }
    }

    /// Cancels unsubmitted resize only; a submitted adjustment belongs to ClientControl.
    #[cfg(windows)]
    pub(crate) fn cancel_divider(&mut self) -> bool {
        if matches!(self.gesture, Gesture::Divider(_)) {
            self.gesture = Gesture::Idle;
            true
        } else {
            false
        }
    }

    #[cfg(windows)]
    pub(crate) fn popup_event(
        &mut self,
        port: &mut (impl TerminalInputPort + ?Sized),
        event: MouseEvent,
    ) {
        if !self.capture {
            return;
        }
        let Some(map) = &self.map else {
            return;
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
                port.popup_mouse(id, *content, event, false);
                if event.action == MouseAction::Release {
                    self.popup_gesture = None;
                }
                return;
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
                    return;
                }
                port.popup_mouse(id, *content, event, false);
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
            HitResolution::Region(HitTarget::PopupClose(id)) => {
                port.popup_mouse(id, Rect::default(), event, true)
            }
            _ => {}
        }
    }

    pub(crate) const fn capture_enabled(&self) -> bool {
        self.capture
    }

    /// An external modal surface consumed this decoded mouse event before
    /// ordinary pointer dispatch. A cancelled link press still owns its
    /// matching release, but the modal remains the only effect owner.
    #[cfg(windows)]
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
        if port.pointer_selection(
            &expected,
            PointerSelectionIntent::Start { click_count, cell },
        ) != PointerSelectionAdmission::Admitted
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
        let _admission =
            port.pointer_selection(&selection.expected, PointerSelectionIntent::Cancel);
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
        match pane_owns_pointer(
            frame.tracking,
            &event,
            frame.alternate_screen && frame.alternate_scroll,
        ) {
            PointerOwner::Pane => {
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
            PointerOwner::Client if plain_primary_press(event) => {
                if probe.selection_ready {
                    let _admitted = self.begin_selection(port, expected, viewport, at, event);
                }
                PointerOutcome::Handled
            }
            PointerOwner::Client | PointerOwner::Host => PointerOutcome::Ignored,
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
                let _admission =
                    port.pointer_selection(&expected, PointerSelectionIntent::Update { cell });
            }
            MouseAction::Release => {
                self.gesture = Gesture::Idle;
                let _admission =
                    port.pointer_selection(&expected, PointerSelectionIntent::End { cell });
            }
            MouseAction::Press => {}
        }
        PointerOutcome::Handled
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
            #[cfg(windows)]
            if let Gesture::Link(link) = &mut self.gesture {
                // Keep a cancelled tombstone until the matching release so
                // re-arming capture cannot reinterpret that release as a
                // fresh pane event.
                link.cancelled = true;
            } else {
                self.gesture = Gesture::Idle;
            }
            #[cfg(not(windows))]
            {
                self.gesture = Gesture::Idle;
            }
            #[cfg(windows)]
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
        #[cfg(windows)]
        if self
            .divider_preview()
            .is_some_and(|drag| !map.exposes_divider(&drag.target))
        {
            self.cancel_divider();
        }
        #[cfg(windows)]
        if let Gesture::Link(link) = &mut self.gesture
            && !link.source_matches(&map)
        {
            // The matching release remains pointer-owned, but a covered or
            // replaced source can no longer hand off an external effect.
            link.cancelled = true;
        }
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
        #[cfg(windows)]
        {
            self.popup_gesture = None;
        }
        #[cfg(windows)]
        if let Gesture::Link(link) = &mut self.gesture {
            link.cancelled = true;
        } else {
            self.gesture = Gesture::Idle;
        }
        #[cfg(not(windows))]
        {
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
            self.cancel_selection(port);
            #[cfg(windows)]
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
            #[cfg(windows)]
            view.machines.cancel_sidebar_scroll();
            return PointerOutcome::Ignored;
        }
        match self.gesture {
            Gesture::Pending(_) => self.pending_event(view, port, &probe, now, event),
            Gesture::Pane(_) => self.pane_event(view, port, &probe, event),
            Gesture::Selection(_) => self.selection_event(port, &probe, event),
            #[cfg(windows)]
            Gesture::Divider(_) => self.divider_event(view, port, event),
            #[cfg(windows)]
            Gesture::Link(_) => self.link_event(port, &probe, event),
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
            let mut gesture_changed = !matches!(self.gesture, Gesture::Idle);
            #[cfg(windows)]
            if let Gesture::Link(link) = &self.gesture {
                gesture_changed = !link.cancelled;
            }
            let changed = gesture_changed || self.wheel.pending != 0;
            self.cancel_selection(port);
            #[cfg(windows)]
            if let Gesture::Link(link) = &mut self.gesture {
                link.cancelled = true;
            } else {
                self.gesture = Gesture::Idle;
            }
            #[cfg(not(windows))]
            {
                self.gesture = Gesture::Idle;
            }
            self.wheel.clear();
            return changed;
        }
        let mut changed = false;
        #[cfg(windows)]
        if self.divider_preview().is_some_and(|drag| {
            view.overlay().is_some()
                || view.resize_mode()
                || view.machines.open
                || self
                    .map
                    .as_ref()
                    .is_none_or(|map| !map.exposes_divider(&drag.target))
        }) {
            changed |= self.cancel_divider();
        }
        #[cfg(windows)]
        if let Some(drag) = self.divider_preview()
            && let Err(notice) = port.pointer_divider_ready(&drag.target)
        {
            changed |= self.cancel_divider();
            view.configuration_notice(notice);
        }
        if matches!(&self.gesture, Gesture::Selection(selection) if !selection.alive(&probe)) {
            changed |= self.cancel_selection(port);
        }
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
        #[cfg(windows)]
        if event.action == MouseAction::Press
            && event.mods == crate::client_view::Modifiers::NONE
            && !view.resize_mode()
            && !view.machines.open
            && self.can_open_action_menu()
        {
            let target = match map.hit(event.column, event.row) {
                HitResolution::Region(HitTarget::ActionMenuButton)
                    if event.button == MouseButton::Left =>
                {
                    Some(crate::action_menu::MenuTarget::Global)
                }
                HitResolution::Region(HitTarget::Tab(tab))
                    if event.button == MouseButton::Right =>
                {
                    Some(crate::action_menu::MenuTarget::Tab(tab.clone()))
                }
                HitResolution::Region(HitTarget::Context(location))
                    if event.button == MouseButton::Right =>
                {
                    Some(crate::action_menu::MenuTarget::Workspace(location.clone()))
                }
                HitResolution::Region(hit)
                    if hit.sidebar_target().is_some()
                        && (event.button == MouseButton::Right
                            || (event.button == MouseButton::Left
                                && matches!(hit, HitTarget::SidebarMenu(_)))) =>
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
        #[cfg(windows)]
        if event.action == MouseAction::Press
            && event.button == MouseButton::Left
            && event.mods == crate::client_view::Modifiers::NONE
            && view.overlay() != Some(crate::client_view::Overlay::ShortcutHelp)
            && !matches!(map.hit(event.column, event.row),
                HitResolution::Region(hit) if hit.sidebar_target().is_some()
                    || matches!(hit, HitTarget::MachineWorkspaceSurface(_)))
        {
            view.machines.leave_sidebar_navigation();
        }
        match map.hit(event.column, event.row) {
            #[cfg(windows)]
            HitResolution::Region(
                HitTarget::ActionMenuButton
                | HitTarget::ActionMenuRow { .. }
                | HitTarget::SettingsRow { .. },
            ) => PointerOutcome::Ignored,
            #[cfg(windows)]
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
            #[cfg(windows)]
            HitResolution::Region(HitTarget::Popup { .. } | HitTarget::PopupClose(_)) => {
                PointerOutcome::Ignored
            }
            HitResolution::Region(HitTarget::OverlaySurface) => PointerOutcome::Ignored,
            #[cfg(windows)]
            HitResolution::Region(HitTarget::SidebarMenu(_)) => PointerOutcome::Ignored,
            #[cfg(windows)]
            HitResolution::Region(HitTarget::MachineProjectRow { target, window }) => {
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
                    MouseButton::Left => PointerOutcome::SidebarProjectRequested(target.clone()),
                    _ => PointerOutcome::Ignored,
                }
            }
            #[cfg(windows)]
            HitResolution::Region(HitTarget::SidebarDisclosure(target)) => {
                if event.action == MouseAction::Press
                    && event.button == MouseButton::Left
                    && event.mods == crate::client_view::Modifiers::NONE
                {
                    PointerOutcome::SidebarProjectRequested(target.clone())
                } else {
                    PointerOutcome::Ignored
                }
            }
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
                        view.machines.leave_sidebar_navigation();
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
                if event.action != MouseAction::Press || event.button != MouseButton::Left {
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
                if event.action != MouseAction::Press || event.button != MouseButton::Left {
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
                #[cfg(windows)]
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

    #[cfg(windows)]
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
            return PointerOutcome::Ignored;
        }
        if self
            .map
            .as_ref()
            .is_none_or(|map| !map.exposes_divider(&drag.target))
        {
            self.cancel_divider();
            return PointerOutcome::Handled;
        }
        if let Err(notice) = port.pointer_divider_ready(&drag.target) {
            self.cancel_divider();
            view.configuration_notice(notice);
            return PointerOutcome::Handled;
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
                    port.pointer_divider_resize(&drag.target, drag.ratio);
                }
            }
            _ => {}
        }
        PointerOutcome::Handled
    }

    #[cfg(windows)]
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
                    let _admission = port.pointer_link(source, link.cell);
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
        match pane_owns_pointer(
            frame.tracking,
            &press,
            frame.alternate_screen && frame.alternate_scroll,
        ) {
            PointerOwner::Pane => {
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
            PointerOwner::Client if plain_primary_press(press) && probe.selection_ready => {
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
            PointerOwner::Client | PointerOwner::Host => {}
        }
        true
    }
}

#[cfg(test)]
mod tests;
