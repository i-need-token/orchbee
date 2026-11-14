//! The hit map and interaction projection of one painted frame.
//!
//! The renderer records every region it actually painted, with the owning feature's exact witness,
//! into a [`HitMapBuilder`]; the shell publishes the finished [`FrameHitMap`] only after a
//! complete successful draw. Pointer input resolves gestures against that published map and hands
//! each candidate frame an immutable [`PointerProjection`] for transient hover and press state.

use ratatui::layout::Rect;

use orchbee_workbench_protocol::WorkbenchTabId;

use crate::client_view::help::HelpControl;
use crate::workbench_state::{PaneRef, TabStripStart};

pub(crate) mod chrome;
pub(crate) mod interaction;
pub(crate) mod list_scroll;
pub(crate) mod modal;
mod presentation;
pub(crate) mod sidebar_reorder;
pub(crate) mod tabs;
pub(crate) mod tooltip;

pub use interaction::InteractionTarget;
pub(crate) use interaction::PointerProjection;
pub(crate) use list_scroll::Target as ListScrollTarget;
pub use presentation::{DrawnPresentationTarget, HeldRestoreTarget, PointerUnboundPane};
pub(crate) use sidebar_reorder::{SidebarReorderDraw, SidebarReorderPaint};
pub(crate) use tabs::TabDragPreview;
pub(crate) use tooltip::{LegendItem, OpenTooltip, RosterItem, TooltipContent, TooltipText};

/// Whether a painted interaction may currently accept its declared gesture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InteractionAvailability {
    Enabled,
    Disabled,
}

/// The explicit primary-button contract declared by a feature renderer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PrimaryContract {
    /// Arm on press and emit the semantic intent on a matching release.
    ActivateOnRelease,
    /// Two complete clicks on the same cell; emits an ActivateOnRelease intent.
    ActivateOnDoubleClick,
    /// Emit one provisional navigation intent on press; release only ends ownership.
    SelectOnPress,
    /// Hover may project here, but a dedicated gesture state machine owns presses.
    Specialized,
}

/// Typed interaction metadata supplied by the feature that owns the target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InteractionSpec {
    pub(crate) primary: PrimaryContract,
    pub(crate) availability: InteractionAvailability,
}

impl InteractionSpec {
    pub(crate) const fn new(
        primary: PrimaryContract,
        availability: InteractionAvailability,
    ) -> Self {
        Self {
            primary,
            availability,
        }
    }

    pub(crate) const fn enabled(primary: PrimaryContract) -> Self {
        Self::new(primary, InteractionAvailability::Enabled)
    }

    pub(crate) const fn disabled(primary: PrimaryContract) -> Self {
        Self::new(primary, InteractionAvailability::Disabled)
    }

    pub(crate) const fn enabled_for_pointer(self) -> bool {
        matches!(self.availability, InteractionAvailability::Enabled)
    }
}

/// The renderer-facing pointer axis. Domain focus, selection, current, checked,
/// availability, and operation state stay independent in the component model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ProjectedPointer {
    #[default]
    Rest,
    Hovered,
    Pressed,
}

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
    CopyControl {
        token: crate::terminal_copy_mode::CopyModeToken,
        control: crate::terminal_copy_mode::CopyControl,
    },
    CopySurface {
        token: crate::terminal_copy_mode::CopySurfaceToken,
        area: Rect,
        origin: crate::terminal_copy_mode::CopyModeCursor,
    },
    /// One control of the Find bar.
    FindControl {
        token: crate::terminal_find::FindToken,
        control: crate::terminal_find::FindControl,
    },
    /// The Find bar's body off its controls: presses stay with the bar.
    FindSurface(crate::terminal_find::FindToken),
    /// The content of the Panel the Find bar searches: a press closes the bar, the wheel scrolls.
    FindContent(crate::terminal_find::FindToken),
    Help {
        page: usize,
        action: HelpControl,
    },
    TabClose {
        token: crate::workbench_state::CloseConfirmationToken,
        confirm: bool,
    },
    PanelClose {
        token: crate::workbench_state::PanelCloseConfirmationToken,
        confirm: bool,
    },
    Rename {
        token: crate::workbench_state::rename_draft::RenameDraftToken,
        submit: bool,
    },
    WorkspaceClose {
        token: crate::workbench_state::workspace_close::WorkspaceCloseToken,
        confirm: bool,
    },
    /// A control of the login overlay painted for one question.
    MachineLogin {
        token: crate::machine_login::Token,
        control: crate::machine_picker::LoginControl,
    },
    /// A control of the notice painted for one offered login.
    MachineWaiting {
        login: orchbee_machine_protocol::Id,
        control: crate::machine_picker::WaitingControl,
    },
    /// A control actually painted by the Workspace setup owner.
    Setup {
        token: crate::workbench_state::workspace_setup::SetupToken,
        control: ModalControl,
    },
    Worktree {
        token: crate::workbench_state::worktree_launch::WorktreeToken,
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
    AgentBoardControl(crate::agent_board::AgentBoardControlTarget),
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
    SidebarHideButton,
    ShortcutHelpButton,
    TabListButton,
    TabListCloseButton,
    Popup {
        id: orchbee_workbench_protocol::WorkbenchRequestId,
        content: Rect,
    },
    PopupClose(crate::independent_popup::DrawnPopupClose),
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
    Presentation(crate::tui_hit_map::DrawnPresentationTarget),
    /// One action of a held restore's placeholder.
    HeldRestore(crate::tui_hit_map::HeldRestoreTarget),
    /// Empty members carry exact structural identity and no runtime incarnation.
    Unbound {
        selected: crate::tui_hit_map::PointerUnboundPane,
        presentation: Option<orchbee_workbench_protocol::TerminalPresentationId>,
    },
    /// Painted overlay body with no action — swallows the click so it
    /// cannot reach the layer beneath.
    OverlaySurface,
    /// One painted Agent notification card, which opens the Agent it announced.
    NotificationCard(crate::client_view::notifications::CardToken),
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
    /// The overflowing sidebar list's scrollbar column: `range` places its thumb on `track` for the
    /// painted `window`.
    SidebarScrollbar {
        track: Rect,
        range: crate::terminal_scrollbar::ScrollbarRange,
        window: crate::machine_picker::SidebarWindow,
    },
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
    /// The Tab strip's toggle while the sidebar is hidden.
    RestoreSidebar,
    /// The sidebar's own toggle while it is visible.
    HideSidebar,
    /// The sidebar's Help control beside Settings.
    ShortcutHelp,
    /// The Mobile bar's current Tab and Tabs button.
    OpenTabList,
    /// The Tab list's way back.
    CloseTabList,
}

/// Finite controls shared by the two capturing pickers. A row index has meaning
/// only together with its immutable painted token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModalControl {
    Choice(usize),
    NextField,
    /// A retained Worktree removal's Stop Tracking button.
    Abandon,
    Submit,
    Cancel,
    /// The frame's close control, which ends a whole dialog even from a follow-up step.
    Close,
    Surface,
}

impl HitTarget {
    pub(super) fn sidebar_target(&self) -> Option<crate::machine_picker::SidebarTarget> {
        use crate::machine_picker::SidebarTarget;
        match self {
            Self::MachineGroupRow { target, .. }
            | Self::SidebarDisclosure(target)
            | Self::SidebarMenu(target)
            | Self::SidebarClose(target)
            | Self::CreateMenu(Some(target)) => Some(target.clone()),
            Self::MachineWorkspaceRow { selected, .. } => match &selected.context {
                Some(crate::startup_directory::ClientStartupTarget::Context(target)) => {
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

    /// The sidebar row a right-press or keyboard row menu opens for. An Agent row opens a menu, but
    /// a press on it still leaves sidebar navigation as it always has, so `sidebar_target` keeps
    /// excluding Agent rows.
    pub(super) fn row_menu_target(&self) -> Option<crate::machine_picker::SidebarTarget> {
        use crate::machine_picker::SidebarTarget;
        match self {
            Self::MachineAgentRow { selected, .. } => match &selected.context {
                Some(crate::startup_directory::ClientStartupTarget::Agent(target)) => {
                    Some(SidebarTarget::Agent {
                        machine: selected.machine.clone(),
                        target: target.clone(),
                    })
                }
                _ => None,
            },
            _ => self.sidebar_target(),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HitRegion {
    pub(super) layer: HitLayer,
    pub(super) rect: Rect,
    pub(super) target: HitTarget,
    pub(super) interaction: Option<interaction::PaintedInteraction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HitResolution<'a> {
    Region(&'a HitTarget),
    /// A capturing overlay was active and the cell hit nothing — an
    /// outside click: dismiss, never fall through.
    OverlayBackdrop,
    /// Nothing claimed the cell.
    Dead,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum HitRegionResolution<'a> {
    Region(&'a HitRegion),
    OverlayBackdrop,
    Dead,
}

pub(crate) fn contains(rect: Rect, column: u16, row: u16) -> bool {
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
    pub(super) sidebar_reorder: Option<SidebarReorderPaint>,
    pub(super) overlay_active: bool,
    pub(super) backdrop_dismissal: Option<HitTarget>,
    pub(super) regions: Vec<HitRegion>,
    pub(super) projection_revision: u64,
    pub(super) modal_projection: modal::Projection,
    pub(super) modal_controls: Vec<modal::Fingerprint>,
    pub(super) chrome_projection: chrome::Projection,
    pub(super) chrome_controls: Vec<chrome::Fingerprint>,
    pub(super) help_draw_source: Option<crate::client_view::help::HelpDrawSource>,
    pub(super) link_sources: Vec<(Rect, crate::terminal_link_activation::PaintedLinkSource)>,
    pub(super) menu_source: Option<crate::action_menu::DrawnMenuSource>,
    pub(super) navigator: Option<crate::client_view::navigator::Paint>,
    pub(super) tab_list: Option<crate::client_view::tab_list::Paint>,
    /// Regions this frame painted with a tooltip; their content stays with the candidate frame.
    pub(super) tooltips: Vec<tooltip::Key>,
    /// Interactions this frame projected the pointer onto in the exposure that pointer recorded,
    /// before it painted the children that pointer reveals. See `revealed_for_pointer`.
    pub(super) pointer_reveals: Vec<interaction::Fingerprint>,
}

impl FrameHitMap {
    pub(crate) fn help_draw_source(&self) -> Option<&crate::client_view::help::HelpDrawSource> {
        self.help_draw_source.as_ref()
    }

    pub(crate) fn navigator_paint(&self) -> Option<&crate::client_view::navigator::Paint> {
        self.navigator.as_ref()
    }

    /// The Tab list rows this frame painted, which keyboard navigation moves through.
    pub(crate) fn tab_list_paint(&self) -> Option<&crate::client_view::tab_list::Paint> {
        self.tab_list.as_ref()
    }

    /// Where a keyboard-opened Tab menu opens beside its row: the row's first cell.
    pub(crate) fn tab_list_anchor(&self, target: &chrome::TabTarget) -> Option<(u16, u16)> {
        let target = target.hit_target();
        self.regions
            .iter()
            .find(|region| region.layer == HitLayer::Overlay && region.target == target)
            .map(|region| (region.rect.x, region.rect.y))
    }
    pub(super) fn hit_region_with<'a>(
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

    pub(super) fn hit_region(&self, column: u16, row: u16) -> HitRegionResolution<'_> {
        self.hit_region_with(None, column, row)
    }

    pub(super) fn hit(&self, column: u16, row: u16) -> HitResolution<'_> {
        match self.hit_region(column, row) {
            HitRegionResolution::Region(region) => HitResolution::Region(&region.target),
            HitRegionResolution::OverlayBackdrop => HitResolution::OverlayBackdrop,
            HitRegionResolution::Dead => HitResolution::Dead,
        }
    }

    pub(super) fn interaction_at(&self, column: u16, row: u16) -> Option<interaction::Fingerprint> {
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

    pub(super) fn modal_fingerprint_for_region(
        &self,
        region: &HitRegion,
    ) -> Option<modal::Fingerprint> {
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
    pub(super) fn backdrop_interaction_at(
        &self,
        column: u16,
        row: u16,
    ) -> Option<interaction::Fingerprint> {
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

    pub(super) fn modal_fingerprint_at(&self, column: u16, row: u16) -> Option<modal::Fingerprint> {
        let HitRegionResolution::Region(region) = self.hit_region(column, row) else {
            return None;
        };
        self.modal_fingerprint_for_region(region)
    }

    pub(super) fn chrome_fingerprint_at(
        &self,
        column: u16,
        row: u16,
    ) -> Option<chrome::Fingerprint> {
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

    pub(super) fn interaction_fingerprint(
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

    pub(super) fn exposure_winner(
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
    pub(super) fn exposure_signature(
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

    pub(super) fn link_source(
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
    pub(super) map: FrameHitMap,
    pub(super) projection: PointerProjection,
    pub(super) covered_sidebar_rows: Vec<Rect>,
    pub(super) tooltip_contents: Vec<(tooltip::Key, TooltipContent)>,
}

impl HitMapBuilder {
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
                    && (region.target.row_menu_target().is_some()
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
    /// Covered chrome supplies placement geometry only, never activation authority. The sidebar's
    /// rows and the Tab list's rows both anchor a menu opened from them.
    pub(crate) fn record_covered_sidebar_geometry(&mut self, painted: Self) {
        self.covered_sidebar_rows.extend(
            painted
                .map
                .regions
                .into_iter()
                .filter(|region| {
                    region.target.row_menu_target().is_some()
                        || matches!(
                            region.target,
                            HitTarget::Tab(_) | HitTarget::TabGroup { .. }
                        )
                })
                .map(|region| region.rect),
        );
    }

    /// Publishes the Tab list rows this frame painted and the window it showed.
    pub(crate) fn record_tab_list(&mut self, paint: crate::client_view::tab_list::Paint) {
        self.map.tab_list = Some(paint);
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
                tab_list: None,
                tooltips: Vec::new(),
                pointer_reveals: Vec::new(),
            },
            projection: PointerProjection::empty(),
            covered_sidebar_rows: Vec::new(),
            tooltip_contents: Vec::new(),
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
        self.projected_tab_pointer_on(HitLayer::Foreground, source, target, rect)
    }

    /// The same projection for a token painted on `layer`, as the Tab list's rows are.
    pub(crate) fn projected_tab_pointer_on(
        &self,
        layer: HitLayer,
        source: &crate::action_menu::DrawnTabCreate,
        target: &HitTarget,
        rect: Rect,
    ) -> ProjectedPointer {
        chrome::Fingerprint::tab_on(layer, source, target, rect)
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
        self.record_tab_target_on(HitLayer::Foreground, source, rect, target);
    }

    /// A Tab or group token on `layer`: the Tab list paints the strip's tokens as rows of the
    /// overlay that covers the window.
    pub(crate) fn record_tab_target_on(
        &mut self,
        layer: HitLayer,
        source: &crate::action_menu::DrawnTabCreate,
        rect: Rect,
        target: HitTarget,
    ) {
        let Some(fingerprint) = chrome::Fingerprint::tab_on(layer, source, &target, rect) else {
            return;
        };
        self.map.regions.push(HitRegion {
            layer,
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
        self.projected_fingerprint(layer, hit_rect, paint_rect, target, spec)
            .map_or(ProjectedPointer::Rest, |fingerprint| {
                self.projection.project(&fingerprint)
            })
    }

    fn projected_fingerprint(
        &self,
        layer: HitLayer,
        hit_rect: Rect,
        paint_rect: Rect,
        target: &InteractionTarget,
        spec: InteractionSpec,
    ) -> Option<interaction::Fingerprint> {
        if hit_rect.is_empty()
            || paint_rect.is_empty()
            || hit_rect.intersection(paint_rect) != hit_rect
        {
            return None;
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
    }

    /// Projects a target as though pointer-revealed frontmost children were
    /// already recorded, in order. A frame that reveals children publishes the
    /// target's exposure with them, so the next frame must project against the
    /// same shape. A projected pointer also marks that exposure as one this
    /// frame recognized and then revealed for the pointer.
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
        let fingerprint = self.projected_fingerprint(layer, hit_rect, paint_rect, target, spec);
        self.map.regions.truncate(recorded);
        let Some(fingerprint) = fingerprint else {
            return ProjectedPointer::Rest;
        };
        let pointer = self.projection.project(&fingerprint);
        if pointer != ProjectedPointer::Rest {
            self.map.pointer_reveals.push(fingerprint);
        }
        pointer
    }

    /// Offers a tooltip on a region this frame records with exactly this layer, rect and target. The
    /// tooltip opens only while that region is frontmost under a resting pointer.
    pub(crate) fn record_tooltip(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        target: HitTarget,
        content: TooltipContent,
    ) {
        if rect.is_empty() {
            return;
        }
        let key = tooltip::Key::new(layer, rect, target);
        self.map.tooltips.push(key.clone());
        self.tooltip_contents.push((key, content));
    }

    /// Offers a tooltip over a pane title's painted text without recording a target there.
    pub(crate) fn record_pane_title_tooltip(
        &mut self,
        rect: Rect,
        pane: PaneRef,
        content: TooltipContent,
    ) {
        if rect.is_empty() {
            return;
        }
        let key = tooltip::Key::pane_title(rect, pane);
        self.map.tooltips.push(key.clone());
        self.tooltip_contents.push((key, content));
    }

    /// Offers a tooltip on part of a region this frame records with exactly this layer, region
    /// rect and target. Record it before any tooltip of the whole region: within `rect` it wins.
    pub(crate) fn record_part_tooltip(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        region: Rect,
        target: &InteractionTarget,
        content: TooltipContent,
    ) {
        self.record_target_part_tooltip(layer, rect, region, target.hit_target(), content);
    }

    /// [`Self::record_part_tooltip`] inside a region recorded by its `HitTarget`, such as a Tab.
    pub(crate) fn record_target_part_tooltip(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        region: Rect,
        target: HitTarget,
        content: TooltipContent,
    ) {
        if rect.is_empty() || region.intersection(rect) != rect {
            return;
        }
        let key = tooltip::Key::part(layer, rect, region, target);
        self.map.tooltips.push(key.clone());
        self.tooltip_contents.push((key, content));
    }

    /// [`Self::record_tooltip`] for a generic interaction recorded at the same rect.
    pub(crate) fn record_interaction_tooltip(
        &mut self,
        layer: HitLayer,
        rect: Rect,
        target: &InteractionTarget,
        content: TooltipContent,
    ) {
        self.record_tooltip(layer, rect, target.hit_target(), content);
    }

    /// Every tooltip this candidate frame offers, in recording order.
    #[cfg(test)]
    pub(crate) fn tooltip_contents(&self) -> impl Iterator<Item = &TooltipContent> {
        self.tooltip_contents.iter().map(|(_, content)| content)
    }

    /// What a pointer resting on this cell of the candidate frame would read.
    #[cfg(test)]
    pub(crate) fn tooltip_content_at(&self, column: u16, row: u16) -> Option<&TooltipContent> {
        let key = self.map.tooltip_at(column, row)?;
        self.tooltip_contents
            .iter()
            .find_map(|(offered, content)| (*offered == key).then_some(content))
    }

    /// The open tooltip when this frame painted its target again, with this frame's content.
    pub(crate) fn open_tooltip(&self) -> Option<OpenTooltip<'_>> {
        let shown = self.projection.tooltip()?;
        if !self.map.paints_tooltip_target(shown) {
            return None;
        }
        self.tooltip_contents
            .iter()
            .rev()
            .find(|(key, _)| key == shown.key())
            .map(|(_, content)| shown.open(content))
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
    pub(super) fn exposes_sidebar_width(
        &self,
        target: &crate::tui_layout::DrawnSidebarWidth,
    ) -> bool {
        let edge = target.edge();
        self.regions.iter().any(|region| {
            region.rect == edge
                && matches!(&region.target, HitTarget::SidebarResize(painted) if painted == target)
        }) && matches!(self.hit(edge.x, edge.y), HitResolution::Region(HitTarget::SidebarResize(painted)) if painted == target)
    }

    /// Whether this frame projected the pointer onto `fingerprint`, exactly as that pointer
    /// recorded it, before painting the children it reveals for that pointer. The interaction it
    /// then recorded at the same place differs from `fingerprint` only by what the reveal added.
    pub(crate) fn revealed_for_pointer(&self, fingerprint: &interaction::Fingerprint) -> bool {
        self.pointer_reveals.contains(fingerprint)
    }

    pub(super) fn exposes_divider(&self, target: &crate::tui_layout::DrawnDivider) -> bool {
        !self.overlay_active
            && self.regions.iter().any(|region| {
                region.layer == HitLayer::Foreground
                    && region.rect == target.divider
                    && matches!(&region.target, HitTarget::Divider(painted) if painted.same_split(target))
            })
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
