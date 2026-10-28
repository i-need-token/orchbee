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
//! - The **interaction projection** explicitly attached to successfully
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
use orchbee_workbench_protocol::WorkbenchTabId;

pub use crate::client_view::help::HelpControl;
use crate::client_view::{ClientAction, ClientView, Modifiers};
use crate::terminal_input::{
    MouseAction, MouseButton, MouseEvent, PaneEncode, PaneMouseEvent, PanePointerRoute,
    PointerFrameFacts, PointerProbe, PointerSelectionAdmission, PointerSelectionCell,
    PointerSelectionIntent, TerminalInputPort, encode_cursor_arrows, encode_pane_report,
    forward_or_report, route_pane_pointer,
};
use crate::workbench_client::state::PaneRef;

mod chrome;
mod copy_mode;
mod interaction;
pub use interaction::{
    InteractionAvailability, InteractionSpec, InteractionTarget, MotionObservability,
    PrimaryContract, ProjectedPointer,
};
pub(crate) use interaction::{PointerProjection, ReleaseOwnership};
mod list_scroll;
mod modal;
pub(crate) use list_scroll::Target as ListScrollTarget;
pub(crate) use modal::CancelledModalEvent;
mod scrollbar;
mod tabs;
pub(crate) use tabs::{TabDragPreview, TabDragSource, TabDragTab, TabStripStart};
mod sidebar_reorder;
#[cfg(test)]
pub(crate) use sidebar_reorder::Placement as SidebarDropPlacement;
pub(crate) use sidebar_reorder::{
    SidebarReorderDraw, SidebarReorderPaint, SidebarReorderPreview, SidebarReorderSource,
};
mod sidebar_width;

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

/// Layers are tested front to back; the first matching region wins within
/// each layer, so specific controls precede their broad surface. A capturing
/// overlay is tested before the layer beneath it, and a
/// press that hits nothing inside it dismisses instead of falling through.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HitLayer {
    /// A noncapturing notification card shields only its painted cells.
    Notification,
    /// The floating status chip occludes only its painted cells without capturing the viewport.
    Status,
    /// Actionable overlay controls — navigator rows. Tested first.
    Overlay,
    /// The painted overlay body itself: inside the overlay but off every
    /// control — the click is swallowed without falling through.
    OverlaySurface,
    /// Sidebar controls remain above panes when height undocks the sidebar.
    Chrome,
    ChromeSurface,
    /// Chrome and pane regions painted under any overlay: tab-strip
    /// tokens, pane leafs.
    Foreground,
}

/// What a recorded region resolves to at click time. Regions carry stable
/// identities, never positions: the probe re-resolves them against current
/// state so a reorder between paint and click cannot misroute.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HitTarget {
    Onboarding(crate::onboarding::Action),
    OnboardingSurface,
    CopyControl {
        token: crate::terminal_copy_mode::CopyModeToken,
        control: crate::terminal_copy_mode::CopyControl,
    },
    CopySurface {
        token: crate::terminal_copy_mode::CopySurfaceToken,
        area: Rect,
        origin: crate::terminal_copy_mode::CopyModeCursor,
    },
    Resize(ResizeControl),
    Help {
        page: usize,
        action: HelpControl,
    },
    TabClose {
        token: crate::workbench_client::CloseConfirmationToken,
        confirm: bool,
    },
    PanelClose {
        token: crate::workbench_client::PanelCloseConfirmationToken,
        confirm: bool,
    },
    Rename {
        token: crate::workbench_client::RenameDraftToken,
        submit: bool,
    },
    WorkspaceClose {
        token: crate::workbench_client::WorkspaceCloseToken,
        confirm: bool,
    },
    /// A control actually painted by the Workspace setup owner.
    Setup {
        token: crate::workbench_client::SetupToken,
        control: ModalControl,
    },
    Worktree {
        token: crate::workbench_client::WorktreeToken,
        control: ModalControl,
    },
    NavigatorRow(crate::client_view::navigator::Row),
    NavigatorClose(u64),
    MachinePicker(crate::machine_picker::DrawnPickerTarget),
    SettingsRow {
        token: crate::native_settings::Token,
        row: crate::native_settings::Row,
    },
    SettingsControl {
        token: crate::native_settings::Token,
        control: crate::native_settings::Control,
    },
    ListScroll(ListScrollTarget),
    AgentBoardControl(crate::workbench_client::AgentBoardControlTarget),
    SettingsButton,
    NewWorkspaceButton,
    CreateMenu(Option<crate::machine_picker::SidebarTarget>),
    TabCreate(crate::action_menu::DrawnTabCreate),
    ActionMenuRow {
        token: crate::action_menu::MenuToken,
        row: usize,
    },
    ActionMenuControl {
        token: crate::action_menu::MenuToken,
        back: bool,
    },
    #[cfg(test)]
    MenuButton,
    /// A visible divider with its exact canonical and geometric source.
    Divider(crate::tui_layout::DrawnDivider),
    SidebarResize(crate::tui_layout::DrawnSidebarWidth),
    SidebarRestoreButton,
    Popup {
        id: orchbee_workbench_protocol::WorkbenchRequestId,
        content: Rect,
    },
    PopupClose(crate::workbench_client::DrawnPopupClose),
    /// A compact tab-strip token (also a non-Windows navigator row).
    Tab(WorkbenchTabId),
    TabCloseRequest {
        source: crate::action_menu::DrawnTabCreate,
        tab: WorkbenchTabId,
    },
    /// A Tab strip edge marker that pages the strip to start at `start`.
    TabStripPage {
        source: crate::action_menu::DrawnTabCreate,
        start: TabStripStart,
    },
    TabGroup {
        tab: WorkbenchTabId,
        group: orchbee_workbench_protocol::WorkbenchTabGroupId,
    },
    /// A non-Windows navigator context row; Windows uses NavigatorRow proofs.
    /// A painted pane leaf. `pane` is the exact incarnation the painter
    /// drew — anchor generation, stable panel, incarnation — so a press
    /// can never migrate onto a re-minted record of the same logical
    /// pane. `content` is its cell viewport — the region a pane program
    /// owns while it reports the pointer; the title row outside it is
    /// Client chrome.
    Pane {
        pane: PaneRef,
        content: Rect,
    },
    /// A dedicated chrome column, disjoint from terminal content and dividers.
    Scrollbar {
        pane: PaneRef,
        content: Rect,
        source: crate::terminal_scrollbar::ScrollbarSnapshot,
    },
    /// Exact painted incarnation and stable destination within that Panel.
    Presentation(crate::workbench_client::DrawnPresentationTarget),
    /// Empty members carry exact structural identity and no runtime incarnation.
    Unbound {
        selected: crate::terminal_input::PointerUnboundPane,
        presentation: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    },
    /// Painted overlay body with no action — swallows the click so it
    /// cannot reach the layer beneath.
    OverlaySurface,
    /// An Agent board row from one exact successfully presented Board view.
    /// Press-time selection validates this complete witness with the owner.
    AgentBoardRow(crate::workbench_client::AgentBoardRowTarget),
    MachineGroupRow {
        target: crate::machine_picker::SidebarTarget,
        window: crate::machine_picker::SidebarWindow,
    },
    SidebarDisclosure(crate::machine_picker::SidebarTarget),
    SidebarMenu(crate::machine_picker::SidebarTarget),
    SidebarClose(crate::machine_picker::SidebarTarget),
    MachineWorkspaceRow {
        selected: crate::machine_picker::Selection,
        window: crate::machine_picker::SidebarWindow,
    },
    MachineWorkspaceSurface(crate::machine_picker::SidebarWindow),
    OpenMachineWorkspaces,
    MachineAgentRow {
        selected: crate::machine_picker::Selection,
        window: crate::machine_picker::SidebarWindow,
    },
    SearchButton,
}

/// Stable Client-owned actions painted in the root chrome. Their live
/// availability is still revalidated by the dispatcher on release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GlobalChromeAction {
    Onboarding(crate::onboarding::Action),
    RestoreSidebar,
}

/// Finite controls shared by the two capturing pickers. A row index has meaning
/// only together with its immutable painted token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalControl {
    Choice(usize),
    NextField,
    Refresh,
    Submit,
    Cancel,
    Surface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResizeControl {
    Adjust {
        target: Box<crate::tui_layout::DrawnResize>,
        direction: crate::client_view::PanelDirection,
        fine: bool,
    },
    Step(bool),
    Help,
    Exit,
}

impl HitTarget {
    fn sidebar_target(&self) -> Option<crate::machine_picker::SidebarTarget> {
        use crate::machine_picker::SidebarTarget;
        match self {
            Self::MachineGroupRow { target, .. }
            | Self::SidebarDisclosure(target)
            | Self::SidebarMenu(target)
            | Self::SidebarClose(target)
            | Self::CreateMenu(Some(target)) => Some(target.clone()),
            Self::MachineWorkspaceRow { selected, .. } => match &selected.context {
                Some(crate::workbench_client::ClientStartupTarget::Context(target)) => {
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
    interaction: Option<interaction::PaintedInteraction>,
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

#[derive(Clone, Copy, Debug)]
enum HitRegionResolution<'a> {
    Region(&'a HitRegion),
    OverlayBackdrop,
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
pub struct FrameHitMap {
    sidebar_reorder: Option<SidebarReorderPaint>,
    overlay_active: bool,
    backdrop_dismissal: Option<HitTarget>,
    regions: Vec<HitRegion>,
    projection_revision: u64,
    modal_projection: modal::Projection,
    modal_controls: Vec<modal::Fingerprint>,
    chrome_projection: chrome::Projection,
    chrome_controls: Vec<chrome::Fingerprint>,
    help_draw_source: Option<crate::client_view::help::HelpDrawSource>,
    link_sources: Vec<(Rect, crate::terminal_link_activation::PaintedLinkSource)>,
    menu_source: Option<crate::action_menu::DrawnMenuSource>,
    navigator: Option<crate::client_view::navigator::Paint>,
}

impl FrameHitMap {
    pub(crate) fn help_draw_source(&self) -> Option<&crate::client_view::help::HelpDrawSource> {
        self.help_draw_source.as_ref()
    }

    pub(crate) fn navigator_paint(&self) -> Option<&crate::client_view::navigator::Paint> {
        self.navigator.as_ref()
    }
    fn hit_region_with<'a>(
        &'a self,
        candidate: Option<&'a HitRegion>,
        column: u16,
        row: u16,
    ) -> HitRegionResolution<'a> {
        if self.overlay_active {
            // A capturing overlay owns the press domain end to end: only
            // its own rows and painted surface resolve, controls before
            // surface. Every other cell — including foreground chrome the
            // overlay does not cover — is a dismiss-only backdrop, never a
            // fall-through to the layer below.
            for layer in [
                HitLayer::Overlay,
                HitLayer::Status,
                HitLayer::OverlaySurface,
            ] {
                for region in self.regions.iter().chain(candidate) {
                    if region.layer == layer && contains(region.rect, column, row) {
                        return HitRegionResolution::Region(region);
                    }
                }
            }
            return HitRegionResolution::OverlayBackdrop;
        }
        for layer in [
            HitLayer::Notification,
            HitLayer::Status,
            HitLayer::Chrome,
            HitLayer::ChromeSurface,
            HitLayer::Foreground,
        ] {
            for region in self.regions.iter().chain(candidate) {
                if region.layer == layer && contains(region.rect, column, row) {
                    return HitRegionResolution::Region(region);
                }
            }
        }
        HitRegionResolution::Dead
    }

    fn hit_region(&self, column: u16, row: u16) -> HitRegionResolution<'_> {
        self.hit_region_with(None, column, row)
    }

    fn hit(&self, column: u16, row: u16) -> HitResolution<'_> {
        match self.hit_region(column, row) {
            HitRegionResolution::Region(region) => HitResolution::Region(&region.target),
            HitRegionResolution::OverlayBackdrop => HitResolution::OverlayBackdrop,
            HitRegionResolution::Dead => HitResolution::Dead,
        }
    }

    fn interaction_at(&self, column: u16, row: u16) -> Option<interaction::Fingerprint> {
        let HitRegionResolution::Region(region) = self.hit_region(column, row) else {
            return None;
        };
        let painted = region.interaction.as_ref()?;
        Some(painted.fingerprint(
            region.layer,
            region.rect,
            self.exposure_signature(painted.paint_rect(), None),
        ))
    }

    fn modal_fingerprint_for_region(&self, region: &HitRegion) -> Option<modal::Fingerprint> {
        let enabled = modal::Fingerprint::from_hit(
            region.layer,
            region.rect,
            &region.target,
            InteractionAvailability::Enabled,
        )?;
        self.modal_controls
            .iter()
            .find(|fingerprint| fingerprint.same_control(&enabled))
            .cloned()
    }

    /// Reuses the exact painted close control without exposing underlying regions.
    fn backdrop_interaction_at(&self, column: u16, row: u16) -> Option<interaction::Fingerprint> {
        if !matches!(self.hit(column, row), HitResolution::OverlayBackdrop) {
            return None;
        }
        let target = self.backdrop_dismissal.as_ref()?;
        let region = self.regions.iter().find(|region| {
            region.layer == HitLayer::Overlay
                && &region.target == target
                && region.interaction.is_some()
        })?;
        self.interaction_fingerprint(region, None)
    }

    fn modal_fingerprint_at(&self, column: u16, row: u16) -> Option<modal::Fingerprint> {
        let HitRegionResolution::Region(region) = self.hit_region(column, row) else {
            return None;
        };
        self.modal_fingerprint_for_region(region)
    }

    fn chrome_fingerprint_at(&self, column: u16, row: u16) -> Option<chrome::Fingerprint> {
        let HitRegionResolution::Region(region) = self.hit_region(column, row) else {
            return None;
        };
        self.chrome_controls
            .iter()
            .find(|fingerprint| {
                fingerprint.matches_region(region.layer, region.rect, &region.target)
            })
            .cloned()
    }

    fn interaction_fingerprint(
        &self,
        region: &HitRegion,
        candidate: Option<&HitRegion>,
    ) -> Option<interaction::Fingerprint> {
        let painted = region.interaction.as_ref()?;
        Some(painted.fingerprint(
            region.layer,
            region.rect,
            self.exposure_signature(painted.paint_rect(), candidate),
        ))
    }

    fn exposure_winner(
        &self,
        candidate: Option<&HitRegion>,
        column: u16,
        row: u16,
    ) -> interaction::ExposureWinner {
        match self.hit_region_with(candidate, column, row) {
            HitRegionResolution::Region(region) => interaction::ExposureWinner::region(
                region.layer,
                region.target.clone(),
                region.interaction.clone(),
            ),
            HitRegionResolution::OverlayBackdrop => interaction::ExposureWinner::OverlayBackdrop,
            HitRegionResolution::Dead => interaction::ExposureWinner::Dead,
        }
    }

    /// Canonicalizes the effective frontmost ownership only inside one
    /// interaction's painted extent. Region edges partition that bounded
    /// area; adjacent tiles with the same semantic winner merge again so a
    /// hidden region cannot perturb the signature merely by adding an edge.
    fn exposure_signature(
        &self,
        paint_rect: Rect,
        candidate: Option<&HitRegion>,
    ) -> interaction::ExposureSignature {
        if paint_rect.is_empty() {
            return interaction::ExposureSignature::default();
        }
        let mut x_edges = vec![paint_rect.x, paint_rect.right()];
        let mut y_edges = vec![paint_rect.y, paint_rect.bottom()];
        for region in self.regions.iter().chain(candidate) {
            let intersection = region.rect.intersection(paint_rect);
            if intersection.is_empty() {
                continue;
            }
            x_edges.extend([intersection.x, intersection.right()]);
            y_edges.extend([intersection.y, intersection.bottom()]);
        }
        x_edges.sort_unstable();
        x_edges.dedup();
        y_edges.sort_unstable();
        y_edges.dedup();

        let mut patches: Vec<interaction::ExposurePatch> = Vec::new();
        let mut previous: Vec<(u16, u16, interaction::ExposureWinner, usize)> = Vec::new();
        for y_pair in y_edges.windows(2) {
            let y = y_pair[0];
            let height = y_pair[1].saturating_sub(y);
            if height == 0 {
                continue;
            }
            let mut runs: Vec<(u16, u16, interaction::ExposureWinner)> = Vec::new();
            for x_pair in x_edges.windows(2) {
                let x = x_pair[0];
                let width = x_pair[1].saturating_sub(x);
                if width == 0 {
                    continue;
                }
                let winner = self.exposure_winner(candidate, x, y);
                if let Some((run_x, run_width, run_winner)) = runs.last_mut()
                    && *run_winner == winner
                    && run_x.saturating_add(*run_width) == x
                {
                    *run_width = run_width.saturating_add(width);
                } else {
                    runs.push((x, width, winner));
                }
            }

            let mut next = Vec::with_capacity(runs.len());
            for (x, width, winner) in runs {
                if let Some((_, _, _, patch_index)) = previous.iter().find(
                    |(previous_x, previous_width, previous_winner, patch_index)| {
                        *previous_x == x
                            && *previous_width == width
                            && *previous_winner == winner
                            && patches[*patch_index].rect.bottom() == y
                    },
                ) {
                    patches[*patch_index].rect.height =
                        patches[*patch_index].rect.height.saturating_add(height);
                    next.push((x, width, winner, *patch_index));
                } else {
                    let patch_index = patches.len();
                    patches.push(interaction::ExposurePatch {
                        rect: Rect::new(x, y, width, height),
                        winner: winner.clone(),
                    });
                    next.push((x, width, winner, patch_index));
                }
            }
            previous = next;
        }
        interaction::ExposureSignature(patches)
    }

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
pub struct HitMapBuilder {
    map: FrameHitMap,
    projection: PointerProjection,
    covered_sidebar_rows: Vec<Rect>,
}

impl HitMapBuilder {
    /// A Tab's whole painted strip token, including its close track whether the control is
    /// revealed or hidden.
    pub(crate) fn tab_area(&self, tab: &WorkbenchTabId) -> Option<Rect> {
        let tab = HitTarget::Tab(tab.clone());
        tabs::tab_regions(&self.map)
            .into_iter()
            .find_map(|(rect, target)| (target == tab).then_some(rect))
    }
    pub fn resolve_sidebar_reorder_preview(
        &self,
        preview: &SidebarReorderPreview,
    ) -> Option<SidebarReorderDraw> {
        preview.resolve(&self.map)
    }
    pub(crate) fn record_sidebar_reorder_paint(&mut self, paint: SidebarReorderPaint) {
        self.map.sidebar_reorder = Some(paint);
    }

    /// A mode toolbar replaces ordinary chrome in exactly the cells it paints.
    pub(crate) fn cover_chrome(&mut self, area: Rect) {
        self.map
            .regions
            .retain(|region| region.rect.intersection(area) != region.rect);
    }
    /// A modal replaces all earlier overlay controls in this frame.
    pub(crate) fn start_modal(&mut self) {
        self.map.backdrop_dismissal = None;
        {
            self.map.navigator = None;
        }
        self.map.overlay_active = true;
        self.map.help_draw_source = None;
        self.map
            .regions
            .retain(|region| !matches!(region.layer, HitLayer::Overlay | HitLayer::OverlaySurface));
    }
    pub(crate) fn start_settings(&mut self) {
        self.map.backdrop_dismissal = None;
        {
            self.map.navigator = None;
        }
        self.map.overlay_active = true;
        self.map.help_draw_source = None;
        self.map
            .regions
            .retain(|region| !matches!(region.layer, HitLayer::Overlay | HitLayer::OverlaySurface));
    }
    pub(crate) fn record_action_menu_source(
        &mut self,
        source: Option<crate::action_menu::DrawnMenuSource>,
    ) {
        self.map.menu_source = source;
    }
    pub(crate) fn context_menu_anchor_rect(&self, column: u16, row: u16) -> Option<Rect> {
        self.map
            .regions
            .iter()
            .filter(|region| {
                contains(region.rect, column, row)
                    && (region.target.sidebar_target().is_some()
                        || matches!(
                            region.target,
                            HitTarget::Tab(_) | HitTarget::TabGroup { .. }
                        ))
            })
            .map(|region| region.rect)
            .chain(self.covered_sidebar_rows.iter().copied())
            .filter(|rect| contains(*rect, column, row))
            .max_by_key(|rect| rect.height)
    }
    /// Covered chrome supplies placement geometry only, never activation authority.
    pub(crate) fn record_covered_sidebar_geometry(&mut self, painted: Self) {
        self.covered_sidebar_rows = painted
            .map
            .regions
            .into_iter()
            .filter(|region| region.target.sidebar_target().is_some())
            .map(|region| region.rect)
            .collect();
    }
    pub(crate) fn start_action_menu(&mut self) {
        self.map.backdrop_dismissal = None;
        self.map.navigator = None;
        self.map.overlay_active = true;
        self.map.help_draw_source = None;
        self.map
            .regions
            .retain(|region| !matches!(region.layer, HitLayer::Overlay | HitLayer::OverlaySurface));
    }
    pub(crate) const fn new() -> Self {
        Self {
            map: FrameHitMap {
                sidebar_reorder: None,
                overlay_active: false,
                backdrop_dismissal: None,
                regions: Vec::new(),
                projection_revision: 0,
                modal_projection: modal::Projection::empty(),
                modal_controls: Vec::new(),
                chrome_projection: chrome::Projection::empty(),
                chrome_controls: Vec::new(),
                help_draw_source: None,
                link_sources: Vec::new(),
                menu_source: None,
                navigator: None,
            },
            projection: PointerProjection::empty(),
            covered_sidebar_rows: Vec::new(),
        }
    }

    /// Starts a candidate frame with an immutable snapshot of the last
    /// authoritative transient pointer presentation.
    pub fn with_pointer(projection: PointerProjection) -> Self {
        let mut builder = Self::new();
        builder.map.projection_revision = projection.revision();
        builder.map.modal_projection = projection.modal().clone();
        {
            builder.map.chrome_projection = projection.chrome().clone();
        }
        builder.projection = projection;
        builder
    }

    /// Discards a rejected candidate frame without discarding the immutable
    /// pointer presentation captured for this draw attempt.
    pub fn reset_candidate(&mut self) {
        let projection = self.projection.clone();
        *self = Self::with_pointer(projection);
    }

    /// A capturing overlay is open this frame: an unresolved press is an
    /// outside click that dismisses it.
    pub(crate) fn mark_overlay_active(&mut self) {
        self.map.overlay_active = true;
    }

    pub(crate) fn start_navigator(
        &mut self,
        source: Option<crate::client_view::navigator::DrawSource>,
        focus: Option<crate::client_view::navigator::Target>,
        page_rows: usize,
    ) {
        self.map.help_draw_source = None;
        self.map.navigator = Some(crate::client_view::navigator::Paint {
            source,
            focus,
            page_rows,
            rows: Vec::new(),
        });
    }

    /// Publishes the exact Help episode and page actually painted by this
    /// candidate frame. Refused fallback surfaces carry their own source and
    /// never inherit paging authority from an older successful frame.
    pub(crate) fn record_help_draw(&mut self, source: crate::client_view::help::HelpDrawSource) {
        self.map.help_draw_source = Some(source);
    }

    /// Only the row painter can publish a complete row's activation proof.
    pub(crate) fn record_navigator_row(
        &mut self,
        rect: Rect,
        row: crate::client_view::navigator::Row,
    ) {
        if rect.height != 1 || rect.width == 0 {
            return;
        }
        if let Some(paint) = &mut self.map.navigator
            && paint.source.as_ref().is_some_and(|source| {
                source.episode() == row.episode && source.source() == &row.source
            })
        {
            paint.rows.push(row.clone());
            self.record_interaction(
                HitLayer::Overlay,
                rect,
                rect,
                InteractionTarget::NavigatorRow(row),
                InteractionSpec::enabled(PrimaryContract::ActivateOnRelease),
            );
        }
    }

    pub(crate) fn record(&mut self, layer: HitLayer, rect: Rect, target: HitTarget) {
        if rect.is_empty() {
            return;
        }
        self.map.regions.push(HitRegion {
            layer,
            rect,
            target,
            interaction: None,
        });
    }

    /// Called only after the current overlay's exact close control was painted.
    pub(crate) fn record_modal_dismissal(&mut self, target: HitTarget) {
        self.map.backdrop_dismissal = Some(target);
    }

    pub(crate) fn record_overlay_dismissal(&mut self, target: &InteractionTarget) {
        self.map.backdrop_dismissal = Some(target.hit_target());
    }

    /// Records one specialized modal control after its complete exact extent
    /// was painted. The ordinary `HitTarget` continues to drive the existing
    /// owner effect; availability is retained only for input admission and
    /// immutable hover/pressed projection.
    pub(crate) fn record_modal_control(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        target: HitTarget,
        availability: InteractionAvailability,
    ) {
        if rect.is_empty() {
            return;
        }
        let Some(fingerprint) = modal::Fingerprint::from_hit(layer, rect, &target, availability)
        else {
            return;
        };
        self.map.regions.push(HitRegion {
            layer,
            rect,
            target,
            interaction: None,
        });
        self.map.modal_controls.push(fingerprint);
    }

    /// Projects the pointer axis for an exact specialized modal target. This
    /// never publishes a generic interaction or changes the target's existing
    /// press/release effect owner.
    pub(crate) fn projected_modal_pointer(
        &self,
        layer: HitLayer,
        rect: Rect,
        target: &HitTarget,
        availability: InteractionAvailability,
    ) -> ProjectedPointer {
        if rect.is_empty() {
            return ProjectedPointer::Rest;
        }
        modal::Fingerprint::from_hit(layer, rect, target, availability)
            .map_or(ProjectedPointer::Rest, |fingerprint| {
                self.projection.project_modal(&fingerprint)
            })
    }

    /// Projects the pointer axis for one exact canonical divider. Drag
    /// ownership remains with the divider gesture state machine.
    pub(crate) fn projected_divider_pointer(
        &self,
        target: &crate::tui_layout::DrawnDivider,
    ) -> ProjectedPointer {
        self.projection
            .project_chrome(&chrome::Fingerprint::divider(target))
    }

    /// Records the exact divider after its complete cells were painted.
    pub(crate) fn record_divider(&mut self, target: crate::tui_layout::DrawnDivider) {
        let fingerprint = chrome::Fingerprint::divider(&target);
        self.map.regions.push(HitRegion {
            layer: HitLayer::Foreground,
            rect: target.divider,
            target: HitTarget::Divider(target),
            interaction: None,
        });
        self.map.chrome_controls.push(fingerprint);
    }

    /// Projects one exact tab or group token from the same workbench source
    /// snapshot that painted the strip. Selection and drag effects remain in
    /// the specialized tab gesture owner.
    pub(crate) fn projected_tab_pointer(
        &self,
        source: &crate::action_menu::DrawnTabCreate,
        target: &HitTarget,
        rect: Rect,
    ) -> ProjectedPointer {
        chrome::Fingerprint::tab(source, target, rect)
            .map_or(ProjectedPointer::Rest, |fingerprint| {
                self.projection.project_chrome(&fingerprint)
            })
    }

    /// Records a successfully painted exact tab/group token. Other targets
    /// are rejected instead of gaining a broad specialized projection path.
    pub(crate) fn record_tab_target(
        &mut self,
        source: &crate::action_menu::DrawnTabCreate,
        rect: Rect,
        target: HitTarget,
    ) {
        let Some(fingerprint) = chrome::Fingerprint::tab(source, &target, rect) else {
            return;
        };
        self.map.regions.push(HitRegion {
            layer: HitLayer::Foreground,
            rect,
            target,
            interaction: None,
        });
        self.map.chrome_controls.push(fingerprint);
    }

    /// Records one feature-declared interaction from the exact geometry its
    /// renderer successfully painted. `hit_rect` may be a child fragment of
    /// a complete row-sized `paint_rect`; frontmost children record first.
    pub(crate) fn record_interaction(
        &mut self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: InteractionTarget,
        spec: InteractionSpec,
    ) {
        if hit_rect.is_empty()
            || paint_rect.is_empty()
            || hit_rect.intersection(paint_rect) != hit_rect
        {
            return;
        }
        let hit_target = target.hit_target();
        self.map.regions.push(HitRegion {
            layer,
            rect: hit_rect,
            target: hit_target,
            interaction: Some(interaction::PaintedInteraction::new(
                target, paint_rect, spec,
            )),
        });
    }

    /// Projects only the transient pointer axis for an exact typed target and
    /// painted extent. The feature retains every persistent component fact.
    pub(crate) fn projected_pointer(
        &self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: &InteractionTarget,
        spec: InteractionSpec,
    ) -> ProjectedPointer {
        if hit_rect.is_empty()
            || paint_rect.is_empty()
            || hit_rect.intersection(paint_rect) != hit_rect
        {
            return ProjectedPointer::Rest;
        }
        let candidate = HitRegion {
            layer,
            rect: hit_rect,
            target: target.hit_target(),
            interaction: Some(interaction::PaintedInteraction::new(
                target.clone(),
                paint_rect,
                spec,
            )),
        };
        self.map
            .interaction_fingerprint(&candidate, Some(&candidate))
            .map_or(ProjectedPointer::Rest, |fingerprint| {
                self.projection.project(&fingerprint)
            })
    }

    /// Projects a target as though pointer-revealed frontmost children were
    /// already recorded, in order. A frame that reveals children publishes the
    /// target's exposure with them, so the next frame must project against the
    /// same shape.
    pub(crate) fn projected_pointer_beneath(
        &mut self,
        layer: HitLayer,
        children: &[(Rect, &InteractionTarget, InteractionSpec)],
        hit_rect: Rect,
        paint_rect: Rect,
        target: &InteractionTarget,
        spec: InteractionSpec,
    ) -> ProjectedPointer {
        let recorded = self.map.regions.len();
        for (child_rect, child_target, child_spec) in children {
            self.record_interaction(
                layer,
                *child_rect,
                *child_rect,
                (*child_target).clone(),
                *child_spec,
            );
        }
        let pointer = self.projected_pointer(layer, hit_rect, paint_rect, target, spec);
        self.map.regions.truncate(recorded);
        pointer
    }

    /// Record the link source from the same terminal paint and pane geometry
    /// as the foreground hit region. It publishes atomically with this map.
    pub(crate) fn record_link_source(
        &mut self,
        rect: Rect,
        source: crate::terminal_link_activation::PaintedLinkSource,
    ) {
        if !rect.is_empty() {
            self.map.link_sources.push((rect, source));
        }
    }

    /// Exact positive-size pane content recorded by this paint pass.
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

    pub fn finish(self) -> FrameHitMap {
        self.map
    }

    /// Only a matching exposed divider can paint a local preview above the current frame.
    pub fn exposes_divider(&self, target: &crate::tui_layout::DrawnDivider) -> bool {
        self.map.exposes_divider(target)
    }

    pub fn exposes_sidebar_width(&self, target: &crate::tui_layout::DrawnSidebarWidth) -> bool {
        self.map.exposes_sidebar_width(target)
    }
}

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
}

impl PointerState {
    /// Immutable transient facts used throughout one candidate render.
    pub(crate) fn projection(&self) -> PointerProjection {
        let projection = self
            .interactions
            .projection()
            .with_modal(self.modal_projection());
        self.sidebar_reorder_projection(projection.with_chrome(self.chrome_projection()))
    }

    /// Applies the settled outer-transport motion level. Downgrade clears
    /// position and click authority before later input can be admitted.
    pub(crate) fn set_motion_observability(&mut self, observability: MotionObservability) {
        if self.motion_observability == observability {
            return;
        }
        self.motion_observability = observability;
        if observability == MotionObservability::ButtonOnly {
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

        interaction || modal || chrome
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
                port.hover_action_menu_row(source, *token, *row)
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
                    port.cancel_action_menu();
                    return PointerOutcome::RegionsChanged;
                }
            }
            MouseButton::Right => {
                port.cancel_action_menu();
                return PointerOutcome::RegionsChanged;
            }
            _ => {}
        }
        PointerOutcome::Ignored
    }

    fn chrome_pressed_fingerprint(&self) -> Option<chrome::Fingerprint> {
        match &self.gesture {
            Gesture::Divider(drag) => Some(chrome::Fingerprint::divider(&drag.projected_target())),
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

    fn chrome_projection(&self) -> chrome::Projection {
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
                port.popup_mouse(id, *content, event);
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
                port.popup_mouse(id, *content, event);
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
        if !enabled {
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
        _capture: Option<crate::tui_composition::CaptureOwner>,
    ) -> bool {
        let probe = port.pointer_probe();
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
                _capture,
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
            return changed;
        }
        let mut changed = false;
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
                    Some(crate::action_menu::MenuTarget::TabCreate(target.clone()))
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
                        port.agent_board_dismiss();
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
                    port.pointer_divider_resize(&drag.target, drag.ratio);
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
                && port.history_wheel(&target.expected, event.button == MouseButton::WheelUp)
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
            port.history_wheel(&target.expected, event.button == MouseButton::WheelUp);
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
                port.history_wheel(&target.expected, event.button == MouseButton::WheelUp);
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

/// Crate-internal `HitMapBuilder` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait HitMapBuilderTestAccess {
    fn start_modal(&mut self);
    fn new() -> Self;
    fn mark_overlay_active(&mut self);
    fn record(&mut self, layer: HitLayer, rect: Rect, target: HitTarget);
    fn record_modal_control(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        target: HitTarget,
        availability: InteractionAvailability,
    );
    fn record_interaction(
        &mut self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: InteractionTarget,
        spec: InteractionSpec,
    );
    fn projected_pointer(
        &self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: &InteractionTarget,
        spec: InteractionSpec,
    ) -> ProjectedPointer;
}

#[cfg(any(test, feature = "test-support"))]
impl HitMapBuilderTestAccess for HitMapBuilder {
    fn start_modal(&mut self) {
        HitMapBuilder::start_modal(self)
    }
    fn new() -> Self {
        HitMapBuilder::new()
    }
    fn mark_overlay_active(&mut self) {
        HitMapBuilder::mark_overlay_active(self)
    }
    fn record(&mut self, layer: HitLayer, rect: Rect, target: HitTarget) {
        HitMapBuilder::record(self, layer, rect, target)
    }
    fn record_modal_control(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        target: HitTarget,
        availability: InteractionAvailability,
    ) {
        HitMapBuilder::record_modal_control(self, layer, rect, target, availability)
    }
    fn record_interaction(
        &mut self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: InteractionTarget,
        spec: InteractionSpec,
    ) {
        HitMapBuilder::record_interaction(self, layer, hit_rect, paint_rect, target, spec)
    }
    fn projected_pointer(
        &self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: &InteractionTarget,
        spec: InteractionSpec,
    ) -> ProjectedPointer {
        HitMapBuilder::projected_pointer(self, layer, hit_rect, paint_rect, target, spec)
    }
}

/// Crate-internal `InteractionSpec` operations the application's tests drive.
#[cfg(any(test, feature = "test-support"))]
pub trait InteractionSpecTestAccess {
    fn enabled(primary: PrimaryContract) -> Self;
}

#[cfg(any(test, feature = "test-support"))]
impl InteractionSpecTestAccess for InteractionSpec {
    fn enabled(primary: PrimaryContract) -> Self {
        InteractionSpec::enabled(primary)
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
